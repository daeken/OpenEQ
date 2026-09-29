use super::*;
use tokio::{
    net::UdpSocket,
    time::{Duration, timeout},
};

async fn peer() -> (ZoneClient, UdpSocket, [u8; 4]) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let client = tokio::spawn(ZoneClient::connect(address, "CampFixture"));
    let mut bytes = [0; 1024];
    let (length, remote) = socket.recv_from(&mut bytes).await.unwrap();
    assert_eq!(length, 14);
    assert_eq!(&bytes[..2], &[0, 1]);
    let code: [u8; 4] = bytes[6..10].try_into().unwrap();
    let mut response = vec![0, 2];
    response.extend_from_slice(&code);
    response.extend_from_slice(&[0; 7]); // key, CRC, encode passes
    response.extend_from_slice(&512u32.to_be_bytes());
    socket.send_to(&response, remote).await.unwrap();
    socket.connect(remote).await.unwrap();
    let zone = client.await.unwrap().unwrap();
    assert_eq!(packet(&socket).await.opcode, ZoneOp::ZoneEntry as u16);
    (zone, socket, code)
}

async fn packet(socket: &UdpSocket) -> AppPacket {
    timeout(Duration::from_secs(2), async {
        let mut bytes = [0; 1024];
        loop {
            let length = socket.recv(&mut bytes).await.unwrap();
            if bytes[..2] == [0, 9] {
                socket.send(&[0, 0x15, bytes[2], bytes[3]]).await.unwrap();
                return AppPacket::decode(&bytes[4..length]).unwrap();
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn camp_is_ready_only_and_sit_cancel_logout_keep_source_wire_order() {
    let (mut zone, socket, code) = peer().await;
    assert!(matches!(zone.camp().await, Err(ZoneError::NotReady)));
    assert!(matches!(
        zone.logout_for_roster().await,
        Err(ZoneError::NotReady)
    ));
    zone.ready = true;
    zone.command(Command::Posture {
        player_id: 7,
        posture: 1,
    })
    .await
    .unwrap();
    zone.camp().await.unwrap();
    zone.command(Command::Posture {
        player_id: 7,
        posture: 0,
    })
    .await
    .unwrap();
    for parameter in [Some(110u32), None, Some(100u32)] {
        let next = packet(&socket).await;
        if let Some(parameter) = parameter {
            assert_eq!(next.opcode, 0x0971);
            assert_eq!(
                next.data,
                [
                    7u16.to_le_bytes().as_slice(),
                    &14u16.to_le_bytes(),
                    &parameter.to_le_bytes()
                ]
                .concat()
            );
        } else {
            assert_eq!(next, AppPacket::empty(0x28ec));
        }
    }
    let leaving = zone.logout_for_roster();
    tokio::pin!(leaving);
    tokio::select! {
        _ = &mut leaving => panic!("sending Logout is not confirmation"),
        next = packet(&socket) => assert_eq!(next, AppPacket::empty(ZoneOp::Logout as u16)),
    }
    socket
        .send(&[&[0, 5], code.as_slice()].concat())
        .await
        .unwrap();
    assert!(leaving.await.is_ok());
}

#[tokio::test]
async fn silence_wrong_session_and_out_of_session_do_not_confirm_logout() {
    for outcome in 0..3 {
        let (mut zone, socket, code) = peer().await;
        zone.ready = true;
        let leaving = zone.logout_for_roster_with_timeout(Duration::from_millis(100));
        tokio::pin!(leaving);
        tokio::select! {
            _ = &mut leaving => panic!("Logout completed before any response"),
            next = packet(&socket) => assert_eq!(next.opcode, ZoneOp::Logout as u16),
        }
        match outcome {
            0 => {}
            1 => {
                let wrong = (u32::from_be_bytes(code) ^ 1).to_be_bytes();
                socket
                    .send(&[&[0, 5], wrong.as_slice()].concat())
                    .await
                    .unwrap();
            }
            _ => {
                socket.send(&[0, 0x1d]).await.unwrap();
            }
        }
        let result = leaving.await;
        if outcome == 2 {
            assert!(matches!(result, Err(ZoneError::LogoutUnconfirmed)));
        } else {
            assert!(matches!(result, Err(ZoneError::LogoutTimeout)));
        }
    }
}

#[tokio::test]
async fn matching_early_close_is_distinct_from_an_intentional_logout() {
    let (mut zone, socket, code) = peer().await;
    zone.ready = true;
    zone.camp().await.unwrap();
    assert_eq!(packet(&socket).await.opcode, 0x28ec);
    socket
        .send(&[&[0, 5], code.as_slice()].concat())
        .await
        .unwrap();
    assert!(matches!(zone.next_event().await, Err(ZoneError::Closed)));
    assert!(zone.peer_disconnected().await);
    assert!(
        zone.logout_for_roster().await.is_err(),
        "closed before Logout cannot become a completed Logout"
    );
}
