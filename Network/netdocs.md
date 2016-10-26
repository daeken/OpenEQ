Fundamental protocol
====================

Connections are all over UDP, using a custom protocol on top.  This protocol provides fragmentation, sequencing, retries, and other things you'd expect from a game protocol.

Each packet type in this network protocol (NP) is either a standalone (e.g. session setup) or part of an application-level packet.  The client reconstructs protocol-level packets to build up the higher level ones.

FP Packet Encoding
------------------

Each FP packet starts with `uint16_t opcode`.  With the exceptions noted below, packets are then followed by an optional byte specifying that the packet is compressed (0x5A) or not compressed (0xA5); if this byte isn't present, packet is not compressed.

The last two bytes of FP packets (again, with exceptions noted below) are a CRC with the key established during session creation.

FP Packets
==========

SessionRequest (0x0001 outgoing)
-------------------------

NOT Compressable  
NO CRC

Very first packet to send upon connection.

	uint32_t unknown; // Always 2
	uint32_t sessionID; // Random number
	uint32_t maxLength; // Always 512

This packet is sent with no special encoding, but all integers are big endian.

Yields: SessionResponse

SessionResponse (0x0002 incoming)
------------------------

NOT Compressable  
NO CRC

	uint32_t sessionID; // Should match the one sent by the client
	uint32_t key; // CRC "key"
	uint8_t unknownA;
	uint8_t format;
	uint8_t unknownB;
	uint32_t maxLength;
	uint32_t unknownC;

Combined (0x0003)
-----------------

This is really a bunch of smaller packets, as the name indicates.  Each subpacket is prefixed with a uint8_t telling you the length, then that many bits follows.

SessionDisconnect (0x0005)
--------------------------

KeepAlive (0x0006)
------------------

SessionStatRequest (0x0007)
---------------------------

SessionStatResponse (0x0008)
----------------------------

Packet (0x0009)
---------------

Fragment (0x000D)
-----------------

Out of Order (0x0011)
---------------------

Ack (0x0015)
------------

	uint16_t seq; // Sequence number to acknowledge

Login
=====

World
=====

Zone
====
