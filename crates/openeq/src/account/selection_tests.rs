//! Local UDP peer exercises the production selection pump; no login account,
//! server database, real character, renderer, or audio device is used.
use super::*;
use crate::account_creation::{
    AppearancePolicy, Draft, Phase, PreviewFamily, PreviewReceipt, Rejection,
};
use openeq_assets::character::customization::CustomizationCatalog;
use openeq_net::{AppPacket, creation, opcodes::WorldOp};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::{net::UdpSocket, time::timeout};

struct Peer {
    socket: UdpSocket,
    code: [u8; 4],
    sequence: u16,
    received: BTreeSet<u16>,
}
impl Peer {
    async fn send(&mut self, packet: AppPacket) {
        let mut data = vec![0, 9];
        data.extend_from_slice(&self.sequence.to_be_bytes());
        self.sequence += 1;
        data.extend(packet.encode());
        self.socket.send(&data).await.unwrap();
    }
    async fn next(&mut self) -> AppPacket {
        let mut data = [0; 2048];
        loop {
            let len = self.socket.recv(&mut data).await.unwrap();
            if len >= 4 && data[..2] == [0, 9] {
                self.socket
                    .send(&[0, 0x15, data[2], data[3]])
                    .await
                    .unwrap();
                let sequence = u16::from_be_bytes([data[2], data[3]]);
                if self.received.insert(sequence) {
                    return AppPacket::decode(&data[4..len]).unwrap();
                }
            }
        }
    }
    async fn packet(&mut self) -> AppPacket {
        timeout(Duration::from_secs(2), self.next()).await.unwrap()
    }
    async fn quiet(&mut self) {
        assert!(
            timeout(Duration::from_millis(100), self.next())
                .await
                .is_err(),
            "unexpected retry or other application mutation"
        );
    }
    async fn close(&self) {
        self.socket
            .send(&[&[0, 5], self.code.as_slice()].concat())
            .await
            .unwrap();
    }
}

fn catalog_packet() -> AppPacket {
    let mut data = vec![0];
    for word in [
        1u32, 17, 71, 72, 73, 74, 75, 76, 77, 1, 2, 3, 4, 5, 6, 7, 1, 0, 1, 2, 201, 17, 77,
    ] {
        data.extend(word.to_le_bytes());
    }
    AppPacket::new(creation::OP_CATALOG, data)
}
fn roster(name: Option<&str>, enabled: bool) -> AppPacket {
    let mut data = u32::from(name.is_some()).to_le_bytes().to_vec();
    if let Some(name) = name {
        data.extend(name.as_bytes());
        data.push(0);
        let mut tail = vec![0; 274];
        tail[0] = 2;
        tail[1..5].copy_from_slice(&1u32.to_le_bytes());
        tail[5] = 1;
        tail[11..13].copy_from_slice(&394u16.to_le_bytes());
        tail[268] = u8::from(enabled);
        data.extend(tail);
    }
    AppPacket::new(WorldOp::SendCharInfo as u16, data)
}
async fn peer() -> (WorldClient, Peer) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let client = tokio::spawn(WorldClient::connect(address, 1, "fixture"));
    let mut bytes = [0; 1024];
    let (length, remote) = socket.recv_from(&mut bytes).await.unwrap();
    assert_eq!(length, 14);
    assert_eq!(&bytes[..2], &[0, 1]);
    let code: [u8; 4] = bytes[6..10].try_into().unwrap();
    let mut response = vec![0, 2];
    response.extend_from_slice(&code);
    response.extend_from_slice(&[0; 7]);
    response.extend_from_slice(&512u32.to_be_bytes());
    socket.send_to(&response, remote).await.unwrap();
    socket.connect(remote).await.unwrap();
    let mut world = client.await.unwrap().unwrap();
    let mut peer = Peer {
        socket,
        code,
        sequence: 0,
        received: BTreeSet::new(),
    };
    assert_eq!(peer.packet().await.opcode, WorldOp::SendLoginInfo as u16);
    let mut expansions = vec![0; 68];
    expansions[64..].copy_from_slice(&0xffffu32.to_le_bytes());
    peer.send(AppPacket::new(creation::OP_EXPANSIONS, expansions))
        .await;
    peer.send(AppPacket::new(
        creation::OP_MAX_CHARACTERS,
        [12u32, 0, 0]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ))
    .await;
    peer.send(AppPacket::new(
        creation::OP_MEMBERSHIP,
        [2u32, 0xffff, 0xffff, 25]
            .into_iter()
            .chain([u32::MAX; 25])
            .flat_map(u32::to_le_bytes)
            .collect(),
    ))
    .await;
    peer.send(roster(None, true)).await;
    assert!(world.characters().await.unwrap().is_empty());
    (world, peer)
}

type Job = tokio::task::JoinHandle<(Result<selection::Exit>, CreationState)>;
struct Fixture {
    controller: AccountController,
    peer: Peer,
    job: Job,
    committing: Arc<AtomicBool>,
}
impl Fixture {
    async fn start() -> Self {
        let (mut world, peer) = peer().await;
        let mut controller = AccountController::default();
        let (requests, mut rx) = tokio::sync::mpsc::channel(8);
        controller.requests = Some(requests);
        controller.view.token = Token {
            attempt: 1,
            revision: 0,
        };
        let mut token = controller.view.token;
        let sender = controller.sender.clone();
        let committing = Arc::new(AtomicBool::new(false));
        let worker = committing.clone();
        let job = tokio::spawn(async move {
            let mut state = CreationState::default();
            let result = selection::run(
                &mut world,
                "Local fixture",
                &mut token,
                &mut rx,
                &sender,
                &worker,
                &mut state,
            )
            .await;
            (result, state)
        });
        let mut fixture = Self {
            controller,
            peer,
            job,
            committing,
        };
        assert_eq!(fixture.peer.packet().await, creation::Catalog::request());
        fixture.peer.send(catalog_packet()).await;
        fixture.wait(|view| view.selection.catalog.is_some()).await;
        fixture
    }
    async fn wait(&mut self, condition: impl Fn(&CreationView) -> bool) {
        timeout(Duration::from_secs(2), async {
            loop {
                self.controller.poll();
                if self
                    .controller
                    .view
                    .creation
                    .as_ref()
                    .is_some_and(&condition)
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
    }
    fn submission(&self) -> Submission {
        let view = self.controller.view.creation.as_ref().unwrap();
        let context = view.context(self.controller.view.token.attempt, 1);
        let catalog = view.selection.catalog.as_ref().unwrap();
        let combination = catalog.combinations()[0];
        let appearance = AppearancePolicy::for_preview(
            1,
            2,
            0,
            PreviewFamily::Luclin,
            &CustomizationCatalog::default(),
            0,
        )
        .unwrap();
        Submission {
            context,
            draft: Draft {
                name: "Asteria".into(),
                choice: combination.choice,
                gender: 0,
                appearance: appearance.default_appearance(),
                stats: catalog
                    .allocation(combination.allocation_index)
                    .unwrap()
                    .default_stats()
                    .unwrap(),
            },
            appearance,
            preview: PreviewReceipt {
                context,
                family: PreviewFamily::Luclin,
                model_loaded: true,
            },
        }
    }
    fn create(&mut self, submission: Submission) -> bool {
        self.controller.action(
            self.controller.view.token,
            Action::Create(Box::new(submission)),
        )
    }
    async fn stop(mut self) {
        self.controller.cancel();
        let (exit, _) = timeout(Duration::from_secs(2), self.job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(exit.unwrap(), selection::Exit::Cancelled);
        assert!(!self.committing.load(Ordering::Relaxed));
    }
}

#[tokio::test]
async fn retained_capabilities_stale_double_and_frozen_pair_end_in_fresh_roster() {
    let mut fixture = Fixture::start().await;
    let view = fixture.controller.view.creation.as_ref().unwrap();
    assert_eq!(view.selection.capabilities.maximum_characters, Some(12));
    assert_eq!(view.selection.capabilities.expansion_mask, Some(0xffff));
    assert!(view.selection.connection > 0 && view.selection.roster_revision > 0);
    let mut stale = fixture.submission();
    stale.context.catalog_revision += 1;
    assert!(fixture.create(stale));
    fixture
        .wait(|view| view.problem.is_some() && !view.pending())
        .await;
    fixture.peer.quiet().await;
    let submitted = fixture.submission();
    let expected = creation::Creation {
        choice: submitted.draft.choice,
        gender: submitted.draft.gender,
        appearance: submitted.draft.appearance,
        stats: submitted.draft.stats,
    }
    .packet()
    .unwrap();
    assert!(fixture.create(submitted.clone()));
    assert!(!fixture.create(submitted.clone()));
    assert_eq!(
        fixture.peer.packet().await,
        creation::approve_name("Asteria", submitted.draft.choice).unwrap()
    );
    fixture
        .wait(|view| view.phase == Some(Phase::AwaitingApproval))
        .await;
    assert!(fixture.committing.load(Ordering::Relaxed));
    // Directly queued duplicates are rejected by the worker as well as UI.
    fixture
        .controller
        .requests
        .as_ref()
        .unwrap()
        .send(Request {
            token: fixture.controller.view.token,
            action: Action::Create(Box::new(submitted)),
        })
        .await
        .unwrap();
    fixture
        .peer
        .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1]))
        .await;
    assert_eq!(fixture.peer.packet().await, expected);
    // EQEmu refreshes capability advertisements before its success roster.
    // They invalidate future drafts, never the already committed frozen pair.
    fixture
        .peer
        .send(AppPacket::new(
            creation::OP_MAX_CHARACTERS,
            [12u32, 0, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
        ))
        .await;
    fixture.peer.send(roster(Some("Asteria"), false)).await;
    fixture
        .wait(|view| matches!(view.phase, Some(Phase::Completed(_))))
        .await;
    assert_eq!(fixture.controller.view.characters[0].zone, 394);
    assert!(!fixture.controller.view.characters[0].enabled);
    assert!(!fixture.controller.action(
        fixture.controller.view.token,
        Action::ChooseCharacter("Asteria".into())
    ));
    fixture.peer.quiet().await;
    fixture.stop().await;
}

#[tokio::test]
async fn closed_ui_finishes_exact_committed_creation_without_publishing_stale_roster() {
    let mut fixture = Fixture::start().await;
    assert!(fixture.create(fixture.submission()));
    assert_eq!(
        fixture.peer.packet().await.opcode,
        creation::OP_APPROVE_NAME
    );
    fixture.controller.cancel();
    fixture
        .peer
        .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1]))
        .await;
    assert_eq!(fixture.peer.packet().await.opcode, creation::OP_CREATE);
    fixture.peer.send(roster(Some("Asteria"), true)).await;
    let (exit, state) = timeout(Duration::from_secs(2), fixture.job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.unwrap(), selection::Exit::Cancelled);
    assert!(matches!(
        state.active().unwrap().phase(),
        Phase::Completed(_)
    ));
    assert!(state.active().unwrap().detached());
    fixture.controller.poll();
    assert_eq!(fixture.controller.view.stage, Stage::Credentials);
    assert!(fixture.controller.view.characters.is_empty());
    assert!(!fixture.committing.load(Ordering::Relaxed));
    fixture.peer.quiet().await;
}

#[tokio::test]
async fn back_before_create_and_old_callback_send_no_reservation() {
    let mut fixture = Fixture::start().await;
    let old = Token {
        attempt: 0,
        revision: 1,
    };
    fixture
        .controller
        .requests
        .as_ref()
        .unwrap()
        .send(Request {
            token: old,
            action: Action::Create(Box::new(fixture.submission())),
        })
        .await
        .unwrap();
    assert!(
        fixture
            .controller
            .action(fixture.controller.view.token, Action::Back)
    );
    assert!(!fixture.create(fixture.submission()));
    let (exit, state) = timeout(Duration::from_secs(2), fixture.job)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exit.unwrap(), selection::Exit::Back);
    assert!(state.active().is_none());
    fixture.peer.quiet().await;
}

#[tokio::test]
async fn capability_refresh_resynchronizes_stale_enter_back_and_create_actions() {
    for case in 0..3 {
        let mut fixture = Fixture::start().await;
        fixture.peer.send(roster(Some("Existing"), true)).await;
        fixture
            .wait(|view| !view.selection.characters.is_empty())
            .await;
        let old = fixture.controller.view.token;
        fixture
            .peer
            .send(AppPacket::new(
                creation::OP_MAX_CHARACTERS,
                [12u32, 0, 0]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            ))
            .await;

        // Wait for the worker to consume the refresh without letting the
        // foreground poll it yet. This is a deterministic overtaking race.
        let refresh = timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(reply) = fixture.controller.replies.lock().unwrap().try_recv() {
                    assert!(reply.token.revision > old.revision);
                    assert!(matches!(reply.event, Event::Creation(_)));
                    break reply;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        let action = match case {
            0 => Action::ChooseCharacter("Existing".into()),
            1 => Action::Back,
            _ => Action::Create(Box::new(fixture.submission())),
        };
        assert!(fixture.controller.action(old, action));
        if case < 2 {
            assert!(fixture.controller.view.stage.busy());
        } else {
            assert!(
                fixture
                    .controller
                    .view
                    .creation
                    .as_ref()
                    .unwrap()
                    .submitting
            );
        }
        assert!(fixture.controller.sender.send(refresh).is_ok());
        fixture
            .wait(|view| {
                view.problem.as_deref()
                    == Some(
                        "Character selection changed. Review the refreshed roster and try again.",
                    )
            })
            .await;
        assert_eq!(fixture.controller.view.stage, Stage::Characters);
        assert!(fixture.controller.view.token.revision > old.revision);
        assert!(!fixture.controller.view.creation.as_ref().unwrap().pending());
        assert!(!fixture.job.is_finished());
        fixture.peer.quiet().await;

        // A fresh explicit action remains usable; nothing entered, navigated,
        // reserved a name, or created a character on behalf of the stale one.
        let retry = if case == 0 {
            Action::ChooseCharacter("Existing".into())
        } else {
            Action::Back
        };
        assert!(
            fixture
                .controller
                .action(fixture.controller.view.token, retry)
        );
        let (exit, state) = timeout(Duration::from_secs(2), fixture.job)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            exit.unwrap(),
            if case == 0 {
                selection::Exit::Character("Existing".into())
            } else {
                selection::Exit::Back
            }
        );
        assert!(state.active().is_none());
    }
}

#[tokio::test]
async fn unknown_malformed_duplicate_approval_and_closed_socket_never_retry_or_delete() {
    for failure in 0..4 {
        let mut fixture = Fixture::start().await;
        assert!(fixture.create(fixture.submission()));
        assert_eq!(
            fixture.peer.packet().await.opcode,
            creation::OP_APPROVE_NAME
        );
        match failure {
            0 => {
                fixture
                    .peer
                    .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![3]))
                    .await
            }
            1 => {
                fixture
                    .peer
                    .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1, 1]))
                    .await
            }
            _ => {
                fixture
                    .peer
                    .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1]))
                    .await;
                assert_eq!(fixture.peer.packet().await.opcode, creation::OP_CREATE);
                if failure == 2 {
                    fixture
                        .peer
                        .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1]))
                        .await;
                } else {
                    fixture.peer.close().await;
                }
            }
        }
        let (exit, state) = timeout(Duration::from_secs(2), fixture.job)
            .await
            .unwrap()
            .unwrap();
        assert!(exit.is_err());
        assert!(matches!(
            state.active().unwrap().phase(),
            Phase::Uncertain(_)
        ));
        assert!(!fixture.committing.load(Ordering::Relaxed));
        fixture.peer.quiet().await;
    }
}

#[tokio::test]
async fn creation_rejection_is_not_success_and_cancel_panel_remains_single_flight() {
    let mut fixture = Fixture::start().await;
    assert!(fixture.create(fixture.submission()));
    assert_eq!(
        fixture.peer.packet().await.opcode,
        creation::OP_APPROVE_NAME
    );
    fixture
        .wait(|view| view.phase == Some(Phase::AwaitingApproval))
        .await;
    let operation = fixture
        .controller
        .view
        .creation
        .as_ref()
        .unwrap()
        .operation
        .unwrap();
    assert!(fixture.controller.action(
        fixture.controller.view.token,
        Action::CancelCreation(operation)
    ));
    assert!(!fixture.create(fixture.submission()));
    fixture
        .peer
        .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![1]))
        .await;
    assert_eq!(fixture.peer.packet().await.opcode, creation::OP_CREATE);
    fixture
        .peer
        .send(AppPacket::new(creation::OP_APPROVE_NAME, vec![0]))
        .await;
    fixture
        .wait(|view| view.phase == Some(Phase::Rejected(Rejection::Creation)))
        .await;
    assert!(fixture.controller.view.creation.as_ref().unwrap().detached);
    assert!(fixture.controller.view.characters.is_empty());
    fixture.peer.quiet().await;
    fixture.stop().await;
}

#[tokio::test]
async fn absolute_creation_timeout_survives_unrelated_world_packets() {
    let mut fixture = Fixture::start().await;
    assert!(fixture.create(fixture.submission()));
    assert_eq!(
        fixture.peer.packet().await.opcode,
        creation::OP_APPROVE_NAME
    );
    // Use the production twenty-second deadline. Continual unrelated traffic
    // must not renew a relative receive timeout or starve transaction expiry.
    let started = std::time::Instant::now();
    let outcome = timeout(Duration::from_secs(23), async {
        loop {
            tokio::select! {
                result = &mut fixture.job => break result.unwrap(),
                _ = tokio::time::sleep(Duration::from_millis(20)) => {
                    fixture.peer.send(AppPacket::empty(0x1234)).await;
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(outcome.0.is_err());
    assert!(started.elapsed() < Duration::from_secs(22));
    assert_eq!(
        outcome.1.active().unwrap().phase(),
        &Phase::Uncertain(account_creation::Uncertainty::TimedOut)
    );
    assert!(!fixture.committing.load(Ordering::Relaxed));
    fixture.peer.quiet().await;
}
