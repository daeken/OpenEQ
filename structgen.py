import sys, yaml
from pprint import pprint

typemap = dict(
	uint8='byte', 
	uint16='ushort', 
	uint32='uint', 
	int8='sbyte', 
	int16='short', 
	int32='int'
)

btypemap = dict(
	uint8='Byte', 
	uint16='UInt16', 
	uint32='UInt32', 
	int8='SByte', 
	int16='Int16', 
	int32='Int32', 
	float='Single'
)

class Type(object):
	def __init__(self, spec):
		if '<' in spec:
			base, gen = spec.split('<', 1)
			gen, rest = gen.split('>', 1)
			self.gen = Type(gen)
			spec = base + rest
		else:
			self.gen = None

		if '[' in spec:
			self.rank = spec.split('[', 1)[1].split(']', 1)[0]
			# XXX: Handle multidimensional arrays
		else:
			self.rank = None

		self.base = spec.split('<', 1)[0].split('[', 1)[0]

	def declare(self):
		if self.base == 'varstring':
			type = 'string'
		elif self.base == 'list':
			assert self.gen is not None
			type = 'List<%s>' % self.gen.declare()
		else:
			type = typemap[self.base] if self.base in typemap else self.base
			if self.base != 'bool' and self.gen is not None: # Bools shouldn't be generic in C#
				type += '<%s>' % self.gen.declare()

		if self.base not in ('string', 'list') and self.rank is not None:
			type += '[]'

		return type

	def pack(self, name, ws='', array=False):
		if not array and self.base != 'string' and self.rank is not None:
			tvar = chr(ord('i') + len(ws) - 3)
			print '%sfor(var %s = 0; %s < %s; ++%s) {' % (ws, tvar, tvar, self.rank, tvar)
			self.pack('%s[%s]' % (name, tvar), ws=ws + '\t', array=True)
			print '%s}' % ws
			return

		if self.base in btypemap:
			print '%sbw.Write(%s);' % (ws, name)
		elif self.base == 'bool':
			print '%sbw.Write((%s) (%s ? 1 : 0));' % (ws, typemap[self.gen.base], name)
		elif self.base == 'string' or self.base == 'varstring':
			print '%sbw.Write(%s.ToBytes(%s));' % (ws, name, self.rank if self.base == 'string' else '')
		else:
			print '%s%s.Pack(bw);' % (ws, name)

	def unpack(self, name, ws='', array=False):
		if not array and self.base != 'string' and self.rank is not None:
			if self.base == 'list':
				print '%s%s = new List<%s>();' % (ws, name, self.gen.declare())
			else:
				print '%s%s = new %s[%s];' % (ws, name, typemap[self.base] if self.base in typemap else self.base, self.rank)
			tvar = chr(ord('i') + len(ws) - 3)
			print '%sfor(var %s = 0; %s < %s; ++%s) {' % (ws, tvar, tvar, self.rank, tvar)
			if self.base == 'list':
				self.gen.unpack(name, ws=ws + '\t', array='list')
			else:
				self.unpack('%s[%s]' % (name, tvar), ws=ws + '\t', array=self.base)
			print '%s}' % ws
			return

		if self.base in btypemap:
			val = 'br.Read%s()' % btypemap[self.base]
		elif self.base == 'bool':
			val = 'br.Read%s() != 0' % btypemap[self.gen.base]
		elif self.base == 'string' or self.base == 'varstring':
			val = 'br.ReadString(%s)' % ('-1' if self.base == 'varstring' else self.rank)
		else:
			val = 'new %s(br)' % self.base

		if array == 'list':
			print '%s%s.Add(%s);' % (ws, name, val)
		else:
			print '%s%s = %s;' % (ws, name, val)

class Struct(object):
	def __init__(self, name, ydef):
		self.name = name
		self.elems = []
		for elem in ydef:
			(type, names), = elem.items()
			type = Type(type)
			for name in names.split(','):
				self.elems.append((name.strip(), type))

	def __repr__(self):
		return 'Struct(%r, %r)' % (self.name, self.elems)

	def declare(self):
		print '\tpublic struct %s : IEQStruct {' % self.name

		for name, type in self.elems:
			print '\t\t%s%s %s;' % ('public ' if name[0].isupper() else '', type.declare(), name)

		if len(list(1 for name, type in self.elems if name[0].isupper())):
			print
			print '\t\tpublic %s(%s) : this() {' % (self.name, ', '.join('%s %s' % (type.declare(), name) for name, type in self.elems if name[0].isupper()))
			for name, type in self.elems:
				if name[0].isupper():
					print '\t\t\tthis.%s = %s;' % (name, name)
			print '\t\t}'

		print
		print '\t\tpublic %s(byte[] data, int offset = 0) : this() {' % self.name
		print '\t\t\tUnpack(data, offset);'
		print '\t\t}'
		print '\t\tpublic %s(BinaryReader br) : this() {' % self.name
		print '\t\t\tUnpack(br);'
		print '\t\t}'

		print '\t\tpublic void Unpack(byte[] data, int offset = 0) {'
		print '\t\t\tusing(var ms = new MemoryStream(data, offset, data.Length - offset)) {'
		print '\t\t\t\tusing(var br = new BinaryReader(ms)) {'
		print '\t\t\t\t\tUnpack(br);'
		print '\t\t\t\t}'
		print '\t\t\t}'
		print '\t\t}'

		print '\t\tpublic void Unpack(BinaryReader br) {'
		for name, type in self.elems:
			type.unpack(name, '\t\t\t')
		print '\t\t}'

		print
		print '\t\tpublic byte[] Pack() {'
		print '\t\t\tusing(var ms = new MemoryStream()) {'
		print '\t\t\t\tusing(var bw = new BinaryWriter(ms)) {'
		print '\t\t\t\t\tPack(bw);'
		print '\t\t\t\t\treturn ms.ToArray();'
		print '\t\t\t\t}'
		print '\t\t\t}'
		print '\t\t}'

		print '\t\tpublic void Pack(BinaryWriter bw) {'
		for name, type in self.elems:
			type.pack(name, '\t\t\t')
		print '\t\t}'

		print '\t}'

sfile = yaml.load(file('structs.yml'))

sdefs = {top : {name : Struct(name, struct) for name, struct in structs.items()} for top, structs in sfile.items()}

nsfiles = dict(
	login='OpenEQ/OpenEQ.Game/Network/LoginPackets.cs', 
	world='OpenEQ/OpenEQ.Game/Network/WorldPackets.cs', 
	zone='OpenEQ/OpenEQ.Game/Network/ZonePackets.cs', 
)

for ns, structs in sdefs.items():
	with file(nsfiles[ns], 'w') as fp:
		sys.stdout = fp
		print '''/*
*       o__ __o       o__ __o__/_   o          o    o__ __o__/_   o__ __o                o    ____o__ __o____   o__ __o__/_   o__ __o      
*      /v     v\     <|    v       <|\        <|>  <|    v       <|     v\              <|>    /   \   /   \   <|    v       <|     v\     
*     />       <\    < >           / \\o      / \  < >           / \     <\             / \         \o/        < >           / \     <\    
*   o/                |            \o/ v\     \o/   |            \o/     o/           o/   \o        |          |            \o/       \o  
*  <|       _\__o__   o__/_         |   <\     |    o__/_         |__  _<|           <|__ __|>      < >         o__/_         |         |> 
*   \\          |     |            / \    \o  / \   |             |       \          /       \       |          |            / \       //  
*     \         /    <o>           \o/     v\ \o/  <o>           <o>       \o      o/         \o     o         <o>           \o/      /    
*      o       o      |             |       <\ |    |             |         v\    /v           v\   <|          |             |      o     
*      <\__ __/>     / \  _\o__/_  / \        < \  / \  _\o__/_  / \         <\  />             <\  / \        / \  _\o__/_  / \  __/>     
*
* THIS FILE IS GENERATED BY structgen.py/structs.yml
* DO NOT EDIT
*
*/'''
		print 'using System.Collections.Generic;'
		print 'using System.IO;'
		print
		print 'namespace OpenEQ.Network {'
		for i, (name, struct) in enumerate(structs.items()):
			struct.declare()
			if i != len(structs) - 1:
				print
		print '}'
