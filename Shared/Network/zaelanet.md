Created by Zaela

PROTOCOL PACKETS
================

Protocol packets come in two major forms:

* Session management
* Encapsulation of "application" packets

Most packets sent between the client and the server are of the second form, wrapping "application"-level packet data with a PROTOCOL HEADER and (optional, configured by the server) PROTOCOL FOOTER.

Session management packets are used for initiating a session, defining its characteristics, and to impose order and reliability by way of monitoring SEQUENCE NUMBERS and responding to them with ACKS or OUT OF ORDER REQUESTS. Session management packets are also used to maintain a session when no other data is actively being sent (such as a user idling at the server or character select screens) via KEEP ALIVE ACKS.

The PROTOCOL HEADER, PROTOCOL FOOTER and the fields of any session management packets are always sent in NETWORK BYTE ORDER.  
Application-level data, including APPLICATION OPCODES, are kept in little-endian format (HOST BYTE ORDER on x86/64).


PROTOCOL HEADER
---------------

The PROTOCOL HEADER is present on every session management packet, and on any encapsulated application packet that is SEQUENCED (application packets may be sent in an UNSEQUENCED manner, without encapsulation, when it is not important that every packet be received or received in order: NPC position and hp percentage updates are an example).

Every PROTOCOL PACKET begins with a 2-byte PROTOCOL OPCODE which defines the purpose of the packet. The length of the PROTOCOL HEADER varies depending on the PROTOCOL OPCODE and on the session settings as defined by the server. Possible PROTOCOL HEADER fields, in order, are:

* PROTOCOL OPCODE [2 bytes], always present
* COMPRESSION FLAG [1 byte], only present if the session is in COMPRESSION MODE, and only on encapsulation packets.
	* A value of `0x5a` (`'Z'`) means COMPRESSED, while a value of `0xa5` means NOT COMPRESSED. If the packet is COMPRESSED, all further header fields and application data will be subject to compression. The PROTOCOL FOOTER is not subject to compression, if present.
* SEQUENCE [2 bytes], always present on encapsulation packets, never present on session management packets; may be compressed.
* TOTAL LENGTH [4 bytes], only present on the first packet in a FRAGMENT SERIES; may be compressed.

PROTOCOL FOOTER
---------------

The PROTOCOL FOOTER consists only of a 2-byte CRC, which is only present if the session is in CRC VALIDATION mode. The server will provide a random CRC key for the client to use in the calculation of CRCs as part of the SESSION RESPONSE.

If the session is in CRC VALIDATION mode, all encapsulation packets should have their CRCs calculated, and it should be calculated after any COMPRESSION is applied. All packet data preceding the PROTOCOL FOOTER itself are used in the calculation of the CRC, including the PROTOCOL HEADER and any application data.

(It's not clear which if any session management packets should have a PROTOCOL FOOTER. In my code only SESSION DISCONNECT is CRC'd; this may be all the EQEmu server code actually checks for.)


SESSION MANAGEMENT
==================

A session is begun with a simple handshake, including a SESSION REQUEST and a SESSION RESPONSE. The SESSION REQUEST packet is always sent by the client to the server, never in the other direction. The client begins a new session in this way whenever it transitions to a new portion of the server, namely Login, World (character select), and any Zones.

(I assume that the client would start new session even if e.g. multiple zones were hosted on the same port, and the client was merely transitioning between them, without technically changing the address it is connecting to. But the EQEmu server hosts each zone on a separate port, so this has never been confirmed as far as I know.)

All session management packets are PACKED so that there are no gaps between fields, as if `#pragma pack(1)` was used.


SESSION REQUEST
---------------

This is the first packet sent by the client. The PROTOCOL HEADER fields (protocolOpcode) are included for clarity.

All fields should be translated to NETWORK BYTE ORDER; values are given as they would appear in HOST BYTE ORDER.

Regarding the maxLength field: this is meant to be the maximum size of UDP datagrams the client is willing to send and receive. This does not include fundamental UDP overhead, but does include overhead from the PROTOCOL HEADER and PROTOCOL FOOTER. Essentially, it's a limit on the length values as specified to the send() or sendto() library calls, and returned by the recv() or recvfrom() calls. In theory the client and server should be able to negotate this value,
but in practice it is always 512.

	struct SessionRequest
	{
		uint16_t    protocolOpcode; // 0x01
		uint32_t    unknown;        // Always 2
		uint32_t    sessionId;      // A random number to identify this session. Only used for disconnecting the session later.
		uint32_t    maxLength;      // See note above.
	};


SESSION RESPONSE
----------------

This packet is sent by the server in response to an accepted SESSION REQUEST from the client.

This packet specifies the session modes, namely whether it is subject to COMPRESSION and/or CRC VALIDATION. Enumerations are provided for the two fields that control this.

(The "encoded" mode is not used in normal play -- it's for the "chat server", which I think might just be the Login Chat functionality that no one has used in 14+ years. I'm not sure about that, though. Functions to encode and decode can be found in the EQEmu netcode.)

How the client responds to this packet varies depending on whether the server is Login, World or Zone.

	enum ValidationMode
	{
		None    = 0,
		Crc     = 2
	};

	enum FilterMode
	{
		None        = 0,
		Compressed  = 1,
		Encoded     = 4
	};

	struct SessionResponse
	{
		uint16_t    protocolOpcode; // 0x02
		uint32_t    sessionId;      // Should match the sessionId that was sent in the SESSION REQUEST
		uint32_t    crcKey;         // The CRC key to be used for this session. Will be 0 if the validation mode is None.
		uint8_t     validationMode; // See the ValidationMode enum.
		uint8_t     filterMode;     // See the FilterMode enum.
		uint8_t     unknownA;
		uint32_t    maxLength;      // The maximum UDP datagram size according to the server. See the discussion under 
									// SESSION REQUEST.
		uint32_t    unknownB;
	};

SESSION DISCONNECT
------------------

Used by the client to indicate that it is disconnecting, or by the server to forcibly disconnect the client. This is the only packet that appears to use the session id that was set during the initial SESSION REQUEST and SESSION RESPONSE handshake.

	struct SessionDisconnect
	{
		uint16_t    protocolOpcode; // 0x05
		uint32_t    sessionId;
	};


SESSION STATS
-------------

These packets are sent between the client and server in order to gauge the speed of the connection between them.  The server (and presumably the client) may throttle how many packets it sends to this client per second depending on the values it observes in this exchange.

These exhanges are initiated by the client. You'll want to do one of these exchanges early on, as by default the EQEmu server appears to expect the connection to be somewhat slow, and throttles it.

(This is very noticable when hosting a server locally -- when connecting to World, the World sends a very long packet full of guild names; this can take serveral seconds by default, or less than 1 second when a session stat request is sent with inflated values. I haven't looked at these packets very much. The code for how the EQEmu server responds to it is a bit ugly.)

This is the packet sent by the client:

	struct SessionStatRequest
	{
		uint16_t    protocolOpcode;     // 0x07
		uint32_t    lastLocalDelta;     // Think the deltas are all in milliseconds (?), more or less self-explanatory
		uint32_t    averageDelta;
		uint32_t    lowDelta;
		uint32_t    highDelta;
		uint32_t    lastRemoteDelta;    // Not sure of the exact definition of "local" and "remote" here
		uint64_t    packetsSent;        // Don't think the Session Request counts; I believe THIS packet is counted
										// by itself, though, weirdly.
		uint64_t    packetsReceived;    // Dunno if UNSEQUENCED packets count for this. May need to consult EQEmu code.
										// Or observe real client behavior.
	};


ACKS and SEQUENCES
------------------

ACK packets are used to respond to SEQUENCE NUMBERS, both by the client and by the server. SEQUENCE NUMBERS are 2-bytes long, and always transmitted in NETWORK BYTE ORDER. SEQUENCE NUMBERS begin at 0 whenever a new session is initiated. SEQUENCE NUMBERS are expected to naturally roll over from 65535 to 0. Only encapsulation packets are SEQUENCED; session management packets are never SEQUENCED.

Whenever the client or server received a SEQUENCED packet, it examines the SEQUENCE NUMBER and compares it to the SEQUENCE NUMBER it expected to receive next. Due to the expectation that SEQUENCE NUMBERS roll over after 65535, care needs to be taken to define a window such that a received SEQUENCE NUMBER of e.g. 5 would be considered "higher" than an expected SEQUENCE NUMBER of e.g. 65530.

* If the SEQUENCE NUMBERS are equal, the packet is the PRESENT packet, and may processed immediately.
* If the received SEQUENCE NUMBER is "higher" (see above) than expected, it is a FUTURE packet, and retained until the expected SEQUENCE NUMBER catches up with it.
* If the received SEQUENCE NUMBER is "lower" than expected, then it is a PAST packet, one that should already have been received and processed. An OUT OF ORDER REQUEST should be sent in response to such packets.
	
(The way the EQEmu server responds to OUT OF ORDER REQUESTS is pretty ugly. From what I recall, it basically vomits up every packet it can remember sending to the client in order, from as far back as is still retained in memory, in hopes that things will eventually get re-sequenced. I don't understand how OUT OF ORDER REQUESTS are supposed to work very well.)


Since UDP sends are not reliable, when the client or server sends a packet, it must retain in under the assumption that it may get lost and have to be re-transmitted. Packets should be indexed under the SEQUENCE NUMBER they were sent with in some way.

When an ACK is received, it contains a SEQUENCE NUMBER. It signals that the packet sent with that SEQUENCE NUMBER -- and any packets that were sent and not yet acked with SEQUENCE NUMBERS "below" the one in the ACK -- have been received. These packets can then be released (notwithstanding any weird extra retaining needed for OUT OF ORDER REQUESTS; see above). It is not necessary to ACK every single SEQUENCED packet that is received, as an entire series of SEQUENCED packets can be ACKed by ACKing the most recent one.

If a packet that is sent has not been ACKed in a certain amount of time (a few seconds) it should be re-sent.

ACKs are also used to keep a session alive. If no packets or ACKs have been sent in a certain amount of time, an ACK packet should be sent indicating the most-recently ACKed SEQUENCE NUMBER (or, equivalently, one less than the SEQUENCE NUMBER that is expected to be received next).

	struct AckPacket
	{
		uint16_t    protocolOpcode; // 0x15
		uint16_t    receivedSequenceNumber;
	};

	struct OutOfOrderRequest
	{
		uint16_t    protocolOpcode;         // 0x11
		uint16_t    expectedSequenceNumber; // Not sure about this
	};


APPLICATION PACKET ENCAPSULATION
================================

The are two types of PROTOCOL packets that encapsulate application packets:

* SINGLE PACKET
* FRAGMENTED PACKET


SINGLE PACKET
-------------

PROTOCOL OPCODE: `0x09`

A single, self-contained, SEQUENCED application packet. Any application packet that is smaller than the maxLength defined for the session (including overhead from the PROTOCOL HEADER and PROTOCOL FOOTER, if any) may appear as this kind of packet.

(In practice it's hard to know the exact threshold for when something will be acceptable to send as a SINGLE PACKET, since Zone and (I think) World always compress packets, making the final lengths of packets unpredicable.)

The PROTOCOL HEADER is immediately followed by an APPLICATION OPCODE [2-bytes, occasionally 3, see below] which is in little-endian byte order (HOST BYTE ORDER for x86/64). Any and all application data which follows the APPLICATION OPCODE will also be in little-endian byte order (perhaps excluding some rare exceptions?).

If the total length of a SINGLE PACKET is beneath a certain threshold (not strictly defined -- I think the EQEmu server goes with 30 or 40 bytes) it will forgo COMPRESSION, if the session is in COMPRESSION MODE, simply to save the overhead of performing COMPRESSION for minimal potential gains.

The APPLICATION OPCODE may occasionally be 3-bytes long. This only happens when the low-order byte of the opcode is 0 (e.g. an opcode of 0x4200 == 00 42 in little-endian memory). When this happens, the opcode will be prepended with an additional 0 byte, so that the first 2 bytes will both be 0 (e.g. 00 42 => 00 00 42 == 0x420000). If the client is sending an APPLICATION OPCODE with a low-order byte of 0, it must prepend the extra 0 byte; if it receives an APPLICATION OPCODE from the server where the first two bytes are both 0, it must shift over by 1 byte to obtain the real opcode. It's not clear why the client and server have to do this; presumably it is intended to avoid an ambiguity.


FRAGMENTED PACKET
-----------------

PROTOCOL OPCODE: `0x0d`

If a packet is being sent that has a total size exceeding the maxLength defined for the session, it must be sent in multiple pieces as a FRAGMENTED PACKET. Packets that exceed the maxLength defined for the session cannot be sent in an UNSEQUENCED manner.

Each packet in a FRAGMENT SERIES represents a segment of the final application packet data. Each is encapsulated separately, with its own complete PROTOCOL HEADER and PROTOCOL FOOTER, and may be individually COMPRESSED.

The first packet in a FRAGMENT SERIES encodes the total length of the application packet data (NOT including any overhead from the PROTOCOL HEADERs or PROTOCOL FOOTERs, but including the APPLICATION OPCODE) as a final, 4-byte field of the PROTOCOL HEADER.

When a FRAGMENTED PACKET is being received, it is important to ensure that all of the segments are received before being combined into the final application packet. However, note that due to a bug the EQEmu server may misreport the total length of the application packet data by 1 byte in some instances (namely the Login server, I believe).


COMBINED PACKETS
================

For the sake of efficiency, the client and server always attempt to combine packets -- mainly ACKs and shorter SINGLE PACKETs -- into one COMBINED PACKET before sending it over the wire.

There are two types of COMBINED PACKETS:

* REGULAR COMBINED
* APPLICATION COMBINED


REGULAR COMBINED
----------------

PROTOCOL OPCODE: `0x03`

The most common COMBINED PACKET type. Any contained SINGLE PACKETs will have their own PROTOCOL OPCODE and SEQUENCE NUMBER, but will not have any individual PROTOCOL FOOTER, will not be individually COMPRESSED, and will not have an individual COMPRESSION FLAG. A REGULAR COMBINED packet may also contain embedded ACK packets.

The COMBINED PACKET's PROTOCOL HEADER consists only of the PROTOCOL OPCODE; it does not have any SEQUENCE NUMBER of its own, only contained SINGLE PACKETs do.

Each embedded packet is preceeded by a 1-byte length value. This length value covers the entire packet, including its PROTOCOL HEADER. SINGLE PACKETs that are longer than 255 bytes with the overhead of the PROTOCOL OPCODE and SEQUENCE NUMBER cannot be included in a REGULAR COMBINED packet.


APPLICATION COMBINED
--------------------

PROTOCOL OPCODE: `0x19`

This packet is kind of weird and I haven't really looked at it. The real clients send this, but I don't believe the EQEmu server ever sends it to the client. It combines SINGLE PACKETs, presumably in cases where one of the embedded packets would not fit into a REGULAR COMBINED packet.

It wouldn't be the end of the world to not bother to support these packets, since the server will never send them out, and everything works fine without it. But presumably some small amount of bandwidth efficiency would be lost without them.