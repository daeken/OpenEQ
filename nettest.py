import pyDes
from binascii import crc32
from random import randrange
from socket import *
from select import select
from struct import pack, unpack
from time import time

class odict(dict):
	def __getattr__(self, name):
		return self[name]

OP = odict(
	SessionReady=0x0001, 
	Login=0x0002, 
	ServerListRequest=0x0004, 
	PlayEverquestRequest=0x000d, 
	PlayEverquestResponse=0x0021, 
	ChatMessage=0x0016,  
	LoginAccepted=0x0017, 
	ServerListResponse=0x0018, 
	Poll=0x0029, 
	EnterChat=0x000f, 
	PollResponse=0x0011
)

OPNames = {v:k for k, v in OP.items()}

def opcodeToName(opcode):
	if opcode in OPNames:
		return OPNames[opcode]
	return 'Unknown_%04x' % opcode

printable = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-=_+{}[];\':"<>?,./`~!@#$%^&*()1234567890'
def hexdump(data):
	dlen = len(data)
	for i in xrange(0, dlen, 16):
		print '%04x ' % i, 
		chars = ''
		for j in xrange(16):
			ws = ' ' if j == 7 else ''
			if i+j >= dlen:
				print '  '+ws,
			else:
				print '%02x%s' % (ord(data[i+j]), ws), 
				chars += ('.' if data[i+j] not in printable else data[i+j]) + ws
		print '|', chars
	print '%04x' % dlen

def calcCrc(data, key):
	crc = crc32(pack('<I', key)) & 0xFFFFFFFF
	crc = crc32(data, crc)
	return crc & 0xFFFF

class Packet(object):
	def __init__(self, opcode, body):
		assert len(body) < 500 # leave room for jesus.
		self.opcode = opcode
		self.body = body
		self.sentAt = None
		self.acked = False

	def bake(self, session):
		packet = pack('>H', 0x0009)
		if session.compressing:
			packet += '\xa5' # not compressed
		self.seq = session.outseq
		packet += pack('>H', session.outseq) # sequence
		if (self.opcode & 0xFF) == 0:
			packet += '\0'
		packet += pack('<H', self.opcode)
		packet += self.body
		if session.validating:
			packet += pack('>H', calcCrc(packet, session.crcKey))
		else:
			packet += pack('>H', 0)
		self.data = packet
		return packet

PACKET_WINDOW = 50

class SessionConnection(object):
	def __init__(self, host, port):
		self.host = host
		self.port = port

		self.expectingSession = False
		self.compressing = False
		self.validating = False

		self.outseq = 0
		self.inseq = 0
		self.outreceived = None

		self.outpackets = [None] * 65536

	def pump(self):
		missed = 0
		while True:
			ready, _, _ = select([self.sock], [], [], 1)
			if not len(ready):
				self.checkResends()
				missed += 1
				if missed == 10:
					break
				continue
			packet = self.sock.recv(512)
			missed = 0

			print
			
			opcode, = unpack('>H', packet[:2])
			if opcode in (0x15, 0x11, 0x09, 0x0d, 0x03, 0x19):
				if self.compressing:
					if ord(packet[2]) == 0x5a:
						print 'Got compressed packet...'
						continue
					else:
						assert ord(packet[2]) == 0xa5
						packet = packet[:2] + packet[3:]

			dlen = len(packet)
			if self.validating:
				pcrc, = unpack('>H', packet[-2:])
				mcrc = calcCrc(packet[:-2], self.crcKey)
				if pcrc != mcrc:
					print 'Got packet with invalid CRC: %04x vs %04x' % (pcrc, mcrc)
					continue
				dlen -= 2

			self.handleBarePacket(packet[:dlen])
	
	def handleBarePacket(self, packet):
		opcode, = unpack('>H', packet[:2])
		if opcode == 0x0002: # SessionResponse
			assert self.expectingSession
			self.expectingSession = False
			sessid, self.crcKey, validationMode, filterMode, unka, maxLength, unkb = unpack('>IIBBBII', packet[2:])
			assert self.sessid == sessid
			self.compressing = bool(filterMode & 1)
			self.validating = bool(validationMode & 2)
			self.handleSessionResponse(packet)
		elif opcode == 0x0009: # SinglePacket
			seq, = unpack('>H', packet[2:4])
			if seq != self.inseq:
				print 'Out of sequence!'
				return
			ack = pack('>HH', 0x0015, self.inseq)
			self.sock.send(ack + pack('>H', calcCrc(ack, self.crcKey)))
			self.inseq += 1
			self.handleAppPacket(packet[4:])
		elif opcode == 0x0015: # Ack
			seq, = unpack('>H', packet[2:4])
			print 'Acked up to %i' % seq
			if self.outreceived is None:
				self.outreceived = seq
				for packet in self.outpackets[:self.outreceived + 1]:
					if packet is not None:
						packet.acked = True
			else:
				if self.outreceived > seq:
					if seq + 65536 - self.outreceived < PACKET_WINDOW:
						print 'Got an ack for a packet outside the window...'
					for packet in self.outpackets[self.outreceived + 1:]:
						if packet is not None:
							packet.acked = True
					for packet in self.outpackets[:seq+1]:
						if packet is not None:
							packet.acked = True
				else:
					for packet in self.outpackets[self.outreceived + 1:seq + 1]:
						if packet is not None:
							packet.acked = True
				self.outreceived = seq
		elif opcode == 0x0003: # Combined
			off = 2
			while off < len(packet):
				slen = ord(packet[off])
				off += 1
				self.handleBarePacket(packet[off:off+slen])
				off += slen
		else:
			print 'Unhandled packet'
			hexdump(packet)

	def handleAppPacket(self, packet):
		off = 1 if packet[0] == '\0' else 0
		opcode, = unpack('<H', packet[off:off+2])

		print 'Application packet, opcode %04x (%s)' % (opcode, opcodeToName(opcode))
		funcname = 'handle' + opcodeToName(opcode)
		if hasattr(self, funcname):
			getattr(self, funcname)(packet)
		else:
			print 'Unhandled:'
			hexdump(packet)

	def checkResends(self):
		curtime = time()
		for packet in self.outpackets:
			if packet is not None and not packet.acked and curtime - packet.sentAt > 5:
				print 'Resending packet %i' % packet.seq
				self.sock.send(packet.data)

	def send(self, packet, ack=False):
		print 'Sending packet:'
		self.sock.send(packet.bake(self))
		hexdump(packet.data)
		packet.sentAt = time()
		self.outpackets[self.outseq] = packet
		self.outseq += 1

	def connect(self):
		self.sock = socket(AF_INET, SOCK_DGRAM)
		self.sock.connect((self.host, self.port))
		self.sock.setblocking(False)

		self.sendSessionRequest()
		self.pump()

	def sendSessionRequest(self):
		self.sessid = randrange(0, 1 << 32)
		packet = pack('>HIII', 0x0001, 2, self.sessid, 512)
		self.sock.send(packet)
		self.expectingSession = True

class LoginConnection(SessionConnection):
	def __init__(self, host, port, username, password):
		super(LoginConnection, self).__init__(host, port)
		self.cryptoblob = self.encrypt(username, password)

	def encrypt(self, username, password):
		buf = username + '\0' + password + '\0'
		k = pyDes.des('\0'*8, pyDes.CBC, '\0'*8, pad='\0', padmode=pyDes.PAD_NORMAL)
		return k.encrypt(buf)

	def handleSessionResponse(self, response):
		print 'Got session response.  Sending SessionReady.'
		packet = Packet(OP.SessionReady, ''.join(map(chr, [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0])))
		self.send(packet)

	def handleChatMessage(self, packet):
		# Do the login
		packet = '\x03\0\0\0\0\x02\0\0\0\0' + self.cryptoblob
		self.send(Packet(OP.Login, packet))

	def handleLoginAccepted(self, packet):
		print 'Got login accepted.'
		if len(packet) < 80:
			print 'Bad login'
		hexdump(packet)

		packet = '\x04' + '\0' * 9
		self.send(Packet(OP.ServerListRequest, packet))

#conn = LoginConnection('localhost', 5998, 'foouser', 'barpassword')
#conn = LoginConnection('login.eqemulator.net', 5998, 'daeken', 'password')
conn.connect()
