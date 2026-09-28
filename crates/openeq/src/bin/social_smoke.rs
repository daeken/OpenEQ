//! Live group and quest-link proof with two dedicated development characters.
//! Usage: social_smoke SOCIAL1_CONFIG SOCIAL2_CONFIG. No credentials are logged.
use anyhow::{Context, ensure};
use openeq::chat_links::{ChatLink, parse_chat};
use openeq::group::GroupState;
use openeq_net::{
    gameplay::{ChatChannel, Command, GameplayEvent},
    session::ConnectionConfig,
    social::{SocialCommand, SocialEvent},
    zone::{Position, Spawn, ZoneClient, ZoneEvent},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    time::Duration,
};

struct Client {
    name: String,
    zone: ZoneClient,
    ready: bool,
    own: Option<u32>,
    hold: Option<Position>,
    spawns: BTreeMap<u32, Spawn>,
    events: Vec<SocialEvent>,
    group: GroupState,
    messages: Vec<String>,
    group_chat: Vec<String>,
    links: Vec<(String, ChatLink)>,
}
impl Client {
    async fn connect(path: &Path) -> anyhow::Result<Self> {
        let config = ConnectionConfig::load(path)?;
        let zone = config.connect().await?;
        Ok(Self {
            name: config.character,
            zone,
            ready: false,
            own: None,
            hold: None,
            spawns: BTreeMap::new(),
            events: Vec::new(),
            group: GroupState::default(),
            messages: Vec::new(),
            group_chat: Vec::new(),
            links: Vec::new(),
        })
    }
    fn message(&mut self, raw: String) {
        let parsed = parse_chat(&raw);
        for link in parsed.links {
            let label = parsed.text[link.range.clone()].to_owned();
            println!("{} link: {label:?} -> {:?}", self.name, link.payload.kind());
            self.links.push((label, link));
        }
        if !parsed.text.is_empty() {
            println!("{} message: {}", self.name, parsed.text);
        }
        self.messages.push(parsed.text);
    }
    fn event(&mut self, event: ZoneEvent) {
        match event {
            ZoneEvent::Ready => self.ready = true,
            ZoneEvent::Spawn(spawn) => {
                if spawn.name == self.name {
                    self.own = Some(spawn.id);
                }
                self.spawns.insert(spawn.id, spawn);
            }
            ZoneEvent::Movement { id, position } => {
                if let Some(spawn) = self.spawns.get_mut(&id) {
                    spawn.position = position;
                }
            }
            ZoneEvent::Despawn(id) => {
                self.spawns.remove(&id);
            }
            ZoneEvent::Gameplay(GameplayEvent::Social(event)) => {
                println!("{} social: {event:?}", self.name);
                self.group.apply(event.clone(), &self.name);
                self.events.push(event);
            }
            ZoneEvent::Gameplay(GameplayEvent::Chat(chat)) => {
                if chat.channel == ChatChannel::Group as u32 {
                    self.group_chat.push(chat.text.clone());
                }
                self.message(chat.text);
            }
            ZoneEvent::Gameplay(GameplayEvent::Message(message)) => {
                if let Some(text) = message.text {
                    self.message(text);
                }
                for argument in message.arguments {
                    self.message(argument);
                }
            }
            _ => {}
        }
    }
    async fn heartbeat(&mut self) -> anyhow::Result<()> {
        if let Some(id) = self.own
            && let Some(position) = self
                .hold
                .or_else(|| self.spawns.get(&id).map(|s| s.position))
        {
            self.zone.send_position(id, position).await?;
        }
        Ok(())
    }
    fn member_names(&self) -> BTreeSet<String> {
        self.group.members.iter().map(|m| m.name.clone()).collect()
    }
    async fn social(&self, command: SocialCommand) -> anyhow::Result<()> {
        self.zone.command(Command::Social(command)).await?;
        Ok(())
    }
    async fn chat(&self, channel: ChatChannel, text: &str) -> anyhow::Result<()> {
        self.zone
            .command(Command::Chat {
                channel,
                target: String::new(),
                text: text.to_owned(),
                language: 0,
            })
            .await?;
        Ok(())
    }
    async fn leave(&self) -> anyhow::Result<()> {
        self.zone
            .target(self.own.context("own spawn missing")?)
            .await?;
        self.social(SocialCommand::Leave {
            character: self.name.clone(),
        })
        .await
    }
}
async fn pump(a: &mut Client, b: &mut Client, seconds: f32) -> anyhow::Result<()> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs_f32(seconds);
    let mut heartbeat = tokio::time::interval(Duration::from_millis(200));
    loop {
        tokio::select! {
            _=tokio::time::sleep_until(deadline)=>break,
            _=heartbeat.tick()=>{a.heartbeat().await?;b.heartbeat().await?;},
            event=a.zone.next_event()=>a.event(event?),
            event=b.zone.next_event()=>b.event(event?),
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("openeq_net=info")
        .init();
    let args: Vec<_> = std::env::args().skip(1).collect();
    ensure!(
        args.len() == 2,
        "usage: social_smoke SOCIAL1_CONFIG SOCIAL2_CONFIG"
    );
    let (mut a, mut b) = tokio::try_join!(
        Client::connect(Path::new(&args[0])),
        Client::connect(Path::new(&args[1]))
    )?;
    pump(&mut a, &mut b, 5.).await?;
    ensure!(
        a.ready && b.ready && a.own.is_some() && b.own.is_some(),
        "both clients must enter zone"
    );
    ensure!(
        a.name == "Fellowship" && b.name == "Companion",
        "use dedicated Fellowship/Companion fixtures"
    );
    // Make retries deterministic without leaving a prior fixture group behind.
    a.leave().await?;
    b.leave().await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.group.members.is_empty() && b.group.members.is_empty(),
        "fixture group cleanup failed"
    );
    a.events.clear();
    b.events.clear();
    a.chat(ChatChannel::Say, "#reload opcodes").await?;
    let reload_deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !a
        .messages
        .iter()
        .any(|m| m.contains("Opcodes reloaded") && m.contains("(202)"))
        && tokio::time::Instant::now() < reload_deadline
    {
        pump(&mut a, &mut b, 0.25).await?;
    }
    ensure!(
        a.messages
            .iter()
            .any(|m| m.contains("Opcodes reloaded") && m.contains("(202)")),
        "PoK opcode reload was not confirmed"
    );
    a.zone.target(b.own.unwrap()).await?;
    a.social(SocialCommand::Invite {
        inviter: a.name.clone(),
        invitee: b.name.clone(),
    })
    .await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(b.events.iter().any(|e|matches!(e,SocialEvent::Invitation{inviter,invitee} if inviter==&a.name && invitee==&b.name)),"invite did not arrive");
    b.social(SocialCommand::Decline {
        inviter: a.name.clone(),
        invitee: b.name.clone(),
    })
    .await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.events
            .iter()
            .any(|e| matches!(e, SocialEvent::InvitationCancelled { .. })),
        "decline did not reach inviter; check RoF2 GroupCancelInvite mapping"
    );
    ensure!(
        a.group.members.is_empty() && b.group.members.is_empty(),
        "decline created membership"
    );
    a.social(SocialCommand::Invite {
        inviter: a.name.clone(),
        invitee: b.name.clone(),
    })
    .await?;
    pump(&mut a, &mut b, 1.).await?;
    b.social(SocialCommand::Accept {
        inviter: a.name.clone(),
        invitee: b.name.clone(),
    })
    .await?;
    pump(&mut a, &mut b, 2.).await?;
    let expected = BTreeSet::from([a.name.clone(), b.name.clone()]);
    ensure!(
        a.member_names() == expected && b.member_names() == expected,
        "server-confirmed group roster missing: {:?} / {:?}",
        a.group.members,
        b.group.members
    );
    ensure!(
        a.group.leader == a.name && b.group.leader == a.name,
        "initial leader mismatch"
    );
    let chat = "OpenEQ social probe: group delivery";
    a.chat(ChatChannel::Group, chat).await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        b.group_chat.iter().any(|m| m == chat),
        "group chat was not delivered to the other client"
    );
    a.social(SocialCommand::MakeLeader {
        character: a.name.clone(),
        leader: b.name.clone(),
    })
    .await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.group.leader == b.name && b.group.leader == b.name,
        "leader transfer not confirmed"
    );
    b.leave().await?;
    pump(&mut a, &mut b, 1.).await?;
    ensure!(
        a.group.members.is_empty() && b.group.members.is_empty(),
        "leave did not dissolve two-person group"
    );
    let npc = a
        .spawns
        .values()
        .find(|s| s.name.starts_with("Aid_Eino"))
        .context("Aid Eino not present")?
        .clone();
    a.zone.target(npc.id).await?;
    // Fixture-only GM move makes this a server-approved reposition, independent
    // of movement anti-warp rules or a prior probe's logout location.
    a.chat(ChatChannel::Say, "#goto").await?;
    a.hold = Some(Position {
        x: npc.position.x,
        y: npc.position.y - 3.,
        z: npc.position.z,
        ..Default::default()
    });
    pump(&mut a, &mut b, 2.).await?;
    a.zone.target(a.own.unwrap()).await?;
    a.chat(ChatChannel::Say, "#loc").await?;
    pump(&mut a, &mut b, 0.5).await?;
    a.zone.target(npc.id).await?;
    a.chat(ChatChannel::Say, "Hail, Aid Eino").await?;
    pump(&mut a, &mut b, 1.5).await?;
    let link = a
        .links
        .iter()
        .rev()
        .find(|(label, _)| label.eq_ignore_ascii_case("help"))
        .context("real NPC quest link missing")?
        .1
        .clone();
    a.zone.command(link.activation()).await?;
    pump(&mut a, &mut b, 1.5).await?;
    ensure!(
        a.messages
            .iter()
            .any(|m| m.contains("meet me this night in the Plane of Nightmares")),
        "quest-link activation did not produce NPC response"
    );
    println!(
        "SOCIAL PROOF: invitation, decline, two-client roster, group chat, leader transfer, leave, real Aid Eino quest-link activation passed"
    );
    Ok(())
}
