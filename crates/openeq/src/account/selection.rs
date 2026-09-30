//! One retained world socket owns both selection and immutable creation. UI
//! cancellation can detach a committed transaction, never abort its pairing.
use super::*;
use crate::account_creation::{Context as CreationContext, Effect, Phase};
use openeq_net::{AppPacket, creation, opcodes::WorldOp, world::CharacterSelection};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationView {
    pub selection: CharacterSelection,
    pub operation: Option<account_creation::Token>,
    pub phase: Option<Phase>,
    pub detached: bool,
    pub submitting: bool,
    pub problem: Option<String>,
}
impl CreationView {
    pub fn pending(&self) -> bool {
        self.submitting
            || matches!(
                self.phase,
                Some(Phase::Prepared | Phase::AwaitingApproval | Phase::AwaitingRoster)
            )
    }
    pub fn context(&self, session: u64, draft_revision: u64) -> CreationContext {
        CreationContext {
            session,
            connection: self.selection.connection,
            catalog_revision: self.selection.catalog_revision,
            roster_revision: self.selection.roster_revision,
            draft_revision,
        }
    }
    fn current(world: &WorldClient, state: &CreationState, problem: Option<String>) -> Self {
        let active = state
            .active()
            .filter(|active| active.token().context.connection == world.selection().connection);
        Self {
            selection: world.selection().clone(),
            operation: active.map(|active| active.token()),
            phase: active.map(|active| active.phase().clone()),
            detached: active.is_some_and(|active| active.detached()),
            submitting: false,
            problem,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Exit {
    Character(String),
    Back,
    Cancelled,
}

fn publish(
    world: &WorldClient,
    name: &str,
    token: Token,
    sender: &mpsc::Sender<Reply>,
    state: &CreationState,
    roster: bool,
    problem: Option<String>,
) -> bool {
    if roster
        && sender
            .send(Reply {
                token,
                event: Event::Characters {
                    world_name: name.into(),
                    characters: world.selection().characters.clone(),
                },
            })
            .is_err()
    {
        return false;
    }
    sender
        .send(Reply {
            token,
            event: Event::Creation(Box::new(CreationView::current(world, state, problem))),
        })
        .is_ok()
}

async fn send_claimed(world: &WorldClient, packet: &AppPacket, deadline: Instant) -> Result<()> {
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), world.send(packet)).await??;
    Ok(())
}

/// `committing` is set before claiming approval and cleared only after its
/// bounded result. The account's outer cancellation guard retains this future
/// while true; closing the request channel then detaches the UI here.
#[allow(clippy::too_many_arguments)]
pub(super) async fn run(
    world: &mut WorldClient,
    world_name: &str,
    token: &mut Token,
    requests: &mut tokio::sync::mpsc::Receiver<Request>,
    sender: &mpsc::Sender<Reply>,
    committing: &AtomicBool,
    state: &mut CreationState,
) -> Result<Exit> {
    world.request_creation_catalog().await?;
    token.revision = token.revision.wrapping_add(1);
    if !publish(world, world_name, *token, sender, state, true, None) {
        return Ok(Exit::Cancelled);
    }
    let mut leaving = None;
    loop {
        let deadline = state
            .active()
            .filter(|active| active.token().context.connection == world.selection().connection)
            .and_then(|active| active.deadline());
        let timer = async {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        let old_roster = world.selection().roster_revision;
        let old_catalog = world.selection().catalog_revision;
        let mut problem = None;
        let mut changed = false;
        let mut resync_roster = false;
        tokio::select! {
            biased;
            packet = world.selection_packet() => {
                match packet {
                    Ok(packet) if packet.opcode == creation::OP_APPROVE_NAME => {
                        let active = state.active_mut().filter(|active| active.token().context.connection == world.selection().connection);
                        let Some(active) = active else {
                            bail!("Unexpected name approval; reconnect before creating a character.");
                        };
                        if !active.pending() {
                            bail!("Unexpected name approval after creation ended; reconnect before creating a character.");
                        }
                        let operation = active.token();
                        if let Effect::SendCreate(packet) = active.approval(operation, creation::decode_approval(&packet.data), Instant::now())
                            && send_claimed(world, &packet, active.deadline().unwrap()).await.is_err()
                        {
                            active.transport_failed(operation);
                        }
                        changed = true;
                    }
                    Ok(packet) if packet.opcode == WorldOp::SendCharInfo as u16 => {
                        if let Some(active) = state.active_mut() {
                            let operation = active.token();
                            if operation.context.connection == world.selection().connection {
                                active.roster(operation, world.selection().roster_revision, &world.selection().characters, Instant::now());
                            }
                        }
                        changed = true;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if let Some(active) = state.active_mut()
                            && active.pending()
                        {
                            active.transport_failed(active.token());
                            let _ = publish(world, world_name, *token, sender, state, false, None);
                            committing.store(false, Ordering::Relaxed);
                            bail!("Character creation outcome is unknown after the connection closed. Sign in again to inspect the roster; the request was not retried.");
                        }
                        return Err(error.into());
                    }
                }
            }
            request = requests.recv(), if leaving.is_none() => {
                let Some(request) = request else {
                    if let Some(active) = state.active_mut() {
                        active.cancel();
                    }
                    if !state.active().is_some_and(|active| active.pending()) {
                        committing.store(false, Ordering::Relaxed);
                        return Ok(Exit::Cancelled);
                    }
                    leaving = Some(Exit::Cancelled);
                    continue;
                };
                // Cancellation identifies the exact committed operation, so
                // a roster/capability update cannot strand a queued cancel.
                if let Action::CancelCreation(operation) = &request.action {
                    if request.token.attempt == token.attempt
                        && let Some(active) = state.active_mut()
                        && active.token() == *operation
                    {
                        active.cancel();
                        changed = true;
                    }
                } else if request.token == *token {
                    match request.action {
                        Action::ChooseCharacter(name) if !state.active().is_some_and(|active| active.pending()) => {
                            if world.selection().characters.iter().any(|character| character.enabled && character.name == name) {
                                return Ok(Exit::Character(name));
                            }
                        }
                        Action::Back => {
                            if let Some(active) = state.active_mut() {
                                active.cancel();
                            }
                            leaving = Some(Exit::Back);
                        }
                        Action::Create(submission) => {
                            let view = CreationView::current(world, state, None);
                            let current = view.context(token.attempt, submission.context.draft_revision);
                            if submission.context != current {
                                problem = Some("The character options changed. Review the current choices before creating.".into());
                            } else if let Some(catalog) = &world.selection().catalog {
                                match state.begin(current, submission.draft, catalog, &world.selection().capabilities,
                                    &world.selection().characters, &submission.appearance, submission.preview) {
                                    Ok(operation) => {
                                        let active = state.active_mut().unwrap();
                                        committing.store(true, Ordering::Relaxed);
                                        let packet = active.take_approval(operation, current, Instant::now()).expect("fresh transaction context");
                                        if send_claimed(world, &packet, active.deadline().unwrap()).await.is_err() {
                                            active.transport_failed(operation);
                                        }
                                    }
                                    Err(error) => problem = Some(match error {
                                        account_creation::BeginError::Busy => "Character creation is already in progress.".into(),
                                        account_creation::BeginError::InspectUncertainResult => "Inspect the uncertain creation result before attempting another character.".into(),
                                        account_creation::BeginError::Invalid(error) => error.to_string(),
                                    }),
                                }
                            } else {
                                problem = Some("The server has not supplied valid character creation choices.".into());
                            }
                            changed = true;
                        }
                        _ => {}
                    }
                } else if request.token.attempt == token.attempt
                    && matches!(request.action, Action::ChooseCharacter(_) | Action::Back | Action::Create(_))
                {
                    // The foreground may already show EnteringZone or
                    // JoiningWorld for this action. A capability/catalog
                    // update can overtake it without replacing the roster;
                    // silently ignoring it would strand that busy screen.
                    // Restore the authoritative selection rather than execute
                    // an action against a snapshot the user has not seen.
                    resync_roster = true;
                    changed = true;
                    problem = Some("Character selection changed. Review the refreshed roster and try again.".into());
                }
            }
            _ = timer => {
                if let Some(active) = state.active_mut() {
                    changed = active.expire(Instant::now());
                }
            }
        }
        // Also inspect elapsed time after unrelated ready traffic: the socket
        // branch is authoritative-first and must not starve an absolute limit.
        if let Some(active) = state.active_mut() {
            changed |= active.expire(Instant::now());
        }
        let roster = world.selection().roster_revision != old_roster;
        if roster || world.selection().catalog_revision != old_catalog {
            token.revision = token.revision.wrapping_add(1);
            changed = true;
        }
        if changed
            && !publish(
                world,
                world_name,
                *token,
                sender,
                state,
                roster || resync_roster,
                problem,
            )
        {
            if let Some(active) = state.active_mut() {
                active.cancel();
            }
            leaving = Some(Exit::Cancelled);
        }
        if let Some(active) = state.active()
            && active.token().context.connection == world.selection().connection
            && matches!(active.phase(), Phase::Uncertain(_))
        {
            committing.store(false, Ordering::Relaxed);
            if let Some(exit) = leaving {
                return Ok(exit);
            }
            bail!(
                "Character creation outcome is unknown. Sign in again to inspect the roster; the request was not retried."
            );
        }
        if !state.active().is_some_and(|active| active.pending()) {
            committing.store(false, Ordering::Relaxed);
            if let Some(exit) = leaving {
                return Ok(exit);
            }
        }
    }
}
