import sys, yaml
from pprint import pprint
from math import ceil

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
	float='Single', 
	double='Double'
)

DEBUGSTRUCTS = ()

class Type(object):
	def __init__(self, struct, spec):
		self.struct = struct
		if '<' in spec:
			base, gen = spec.split('<', 1)
			gen, rest = gen.split('>', 1)
			self.gen = Type(self.struct, gen)
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
		if self.base == 'skip':
			return None
		elif self.base == 'varstring':
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
		if self.base == 'skip':
			print '%sbw.Write(new byte[%s]);' % (ws, self.rank)
			return

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
		elif self.base in allEnums:
			print '%sbw.Write((%s) %s);' % (ws, allEnums[self.base].cast, name)
		else:
			print '%s%s.Pack(bw);' % (ws, name)

	def unpack(self, name, ws='', array=False):
		if self.base == 'skip':
			print '%sbr.ReadBytes(%s);' % (ws, self.rank)
			return

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

		if array != 'list' and self.struct.name in DEBUGSTRUCTS:
			print '%sSystem.Console.WriteLine($"Reading field `%s` from { br.BaseStream.Position }");' % (ws, name)
		if self.base in btypemap:
			val = 'br.Read%s()' % btypemap[self.base]
		elif self.base == 'bool':
			val = 'br.Read%s() != 0' % btypemap[self.gen.base]
		elif self.base == 'string' or self.base == 'varstring':
			val = 'br.ReadString(%s)' % ('-1' if self.base == 'varstring' else self.rank)
		elif self.base in allEnums:
			val = '((%s) 0).Unpack(br)' % self.base
		else:
			val = 'new %s(br)' % self.base

		if array == 'list':
			print '%s%s.Add(%s);' % (ws, name, val)
		else:
			print '%s%s = %s;' % (ws, name, val)
			if self.struct.name in DEBUGSTRUCTS:
				print '%sSystem.Console.WriteLine($"Read `%s` { %s }");' % (ws, name, name)

class Struct(object):
	def __init__(self, name, ydef):
		self.name = name
		self.elems = []
		self.suppress = []
		for elem in ydef:
			(type, names), = elem.items()
			if '@' in type:
				type, cond = type.split('@', 2)
				assert cond.startswith('if<') and cond.endswith('>')
				cond = cond[3:-1]
			else:
				cond = None
			type = Type(self, type)
			for name in names.split(','):
				name = name.strip()
				if name.startswith('$'):
					name = name[1:]
					self.suppress.append(name)
				self.elems.append((name, type, cond))

	def __repr__(self):
		return 'Struct(%r, %r)' % (self.name, self.elems)

	def declare(self):
		print '\tpublic struct %s : IEQStruct {' % self.name

		for name, type, cond in self.elems:
			stype = type.declare()
			if stype is None:
				continue
			print '\t\t%s%s %s;' % ('public ' if name[0].isupper() else '', stype, name)

		if len(list(1 for name, type, cond in self.elems if name[0].isupper())):
			print
			print '\t\tpublic %s(%s) : this() {' % (self.name, ', '.join('%s %s' % (type.declare(), name) for name, type, cond in self.elems if name[0].isupper()))
			for name, type, cond in self.elems:
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
		for name, type, cond in self.elems:
			if cond is not None:
				print '\t\t\tif(%s) {' % cond
				type.unpack(name, '\t\t\t\t')
				print '\t\t\t}'
			else:
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
		for name, type, cond in self.elems:
			if cond is not None:
				print '\t\t\tif(%s) {' % cond
				type.pack(name, '\t\t\t\t')
				print '\t\t\t}'
			else:
				type.pack(name, '\t\t\t')
		print '\t\t}'

		print
		print '\t\tpublic override string ToString() {'
		print '\t\t\tvar ret = "struct %s {\\n";' % self.name
		for i, (name, type, cond) in enumerate(self.elems):
			dec = type.declare()
			if dec is None or not name[0].isupper() or name in self.suppress:
				continue

			if cond is not None:
				print '\t\t\tif(%s) {' % cond
				ws = '\t\t\t\t'
			else:
				ws = '\t\t\t'

			print ws + 'ret += "\\t%s = ";' % name
			print ws + 'try {'
			oldws = ws
			ws += '\t'
			if type.base != 'string' and type.rank is not None:
				print ws + 'ret += "{\\n";'
				print ws + 'for(int i = 0, e = %s.%s; i < e; ++i)' % (name, 'Count' if type.base == 'list' else 'Length')
				print ws + '\tret += $"\\t\\t{ Indentify(%s[i], 2) }" + (i != e - 1 ? "," : "") + "\\n";' % name
				print ws + 'ret += "\\t}%s\\n";' % (',' if i != len(self.elems) - 1 else '')
			else:
				print ws + 'ret += $"{ Indentify(%s) }%s\\n";' % (name, ',' if i != len(self.elems) - 1 else '')
			ws = oldws
			print ws + '} catch(NullReferenceException) {'
			print ws + '\tret += "!!NULL!!\\n";'
			print ws + '}'

			if cond is not None:
				print '\t\t\t}'

		print '\t\t\treturn ret + "}";'
		print '\t\t}'

		print '\t}'


class BitType(object):
	def __init__(self, bf, spec):
		self.bf = bf
		assert '<' in spec
		base, size = spec.split('<', 1)
		size, rest = size.split('>', 1)
		self.size = int(size)
		spec = base + rest

		if '[' in spec:
			self.rank = int(spec.split('[', 1)[1].split(']', 1)[0])
			# XXX: Handle multidimensional arrays
		else:
			self.rank = None

		self.bits = self.size * self.rank if self.rank is not None else self.size

		self.base = spec.split('<', 1)[0].split('[', 1)[0]

	def declare(self, new=False, noArray=False):
		if self.base == 'skip':
			return None

		type = self.base
		if type in ('int', 'uint'):
			if self.size <= 8:
				type = 'byte' if type == 'uint' else 'sbyte'
			elif self.size <= 16:
				type = 'ushort' if type == 'uint' else 'short'
			elif self.size > 32:
				print 'A bitfield can only be up to 32 bits (in %s)' % self.bf.name
				assert False

		if not noArray and self.rank is not None:
			type += '[]' if not new else '[%i]' % self.rank

		return type

	def pack(self, name, ws='', array=False):
		assert False # XXX: Implement

		if self.base == 'skip':
			return

		if not array and self.rank is not None:
			tvar = chr(ord('i') + len(ws) - 3)
			print '%sfor(var %s = 0; %s < %s; ++%s) {' % (ws, tvar, tvar, self.rank, tvar)
			self.pack('%s[%s]' % (name, tvar), ws=ws + '\t', array=True)
			print '%s}' % ws
			return

		#

	def unpack(self, name, data=None, ws='', array=False):
		if self.base == 'skip':
			return

		if not array and self.rank is not None:
			print '%s%s = new %s;' % (ws, name, self.declare(new=True))
			tvar = chr(ord('i') + len(ws) - 3)
			print '%sfor(var %s = 0; %s < %s; ++%s) {' % (ws, tvar, tvar, self.rank, tvar)
			self.unpack('%s[%s]' % (name, tvar), ws=ws + '\t', array=True)
			print '%s}' % ws
			return

		val = '%s & 0x%X' % (data, (1 << self.size) - 1)
		if self.base == 'int':
			val = '((int) (%s) ^ 0x%x) - 0x%x' % (val, 1 << (self.size - 1), 1 << (self.size - 1))
		elif self.base == 'bool':
			val = '(%s) != 0' % val

		print '%s%s = (%s) (%s);' % (ws, name, self.declare(noArray=True), val)

class Bitfield(object):
	def __init__(self, name, ydef):
		self.name = name
		self.elems = []
		self.suppress = []
		for elem in ydef:
			(type, names), = elem.items()
			type = BitType(self, type)
			for name in names.split(','):
				name = name.strip()
				if name.startswith('$'):
					name = name[1:]
					self.suppress.append(name)
				self.elems.append((name, type))

		self.bits = sum(type.bits for name, type in self.elems)

	def __repr__(self):
		return 'Bitfield(%r, %r)' % (self.name, self.elems)

	def declare(self):
		print '\tpublic struct %s : IEQStruct {' % self.name

		for name, type in self.elems:
			stype = type.declare()
			if stype is None:
				continue
			print '\t\t%s%s %s;' % ('public ' if name[0].isupper() else '', stype, name)

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
		total_bytes = int(ceil(self.bits / 8.))
		total_bits = total_bytes * 8
		cur_size = min(total_bits, 64)
		while True:
			redo = False
			cur_off = 0
			cur_read = 0
			total_read = 0

			for name, type in self.elems:
				while type.bits > cur_read - cur_off:
					gap = cur_size - (cur_read - cur_off)
					gap_read = int(ceil(gap / 8.)) * 8
					gap_read = min(gap_read, total_bits - total_read)
					if type.bits > gap_read + (cur_read - cur_off):
						assert cur_size != 64
						cur_size *= 2
						redo = True
						break
					else:
						cur_read += gap_read - cur_off
						cur_off = 0
						total_read += gap_read
						assert total_read <= total_bits
				if redo:
					break
				cur_off += type.bits
			if not redo:
				break

		cur_off = 0
		cur_read = 0
		total_read = 0
		print '\t\t\t%s _databuf;' % {8 : 'ubyte', 16 : 'ushort', 32 : 'uint', 64 : 'ulong'}[cur_size]
		for name, type in self.elems:
			if type.bits > cur_read - cur_off:
				gap = cur_size - (cur_read - cur_off)
				gap_read = int(ceil(gap / 8.)) * 8
				gap_read = min(gap_read, total_bits - total_read)
				unit = {8 : 'Byte', 16 : 'UInt16', 32 : 'UInt32', 64 : 'UInt64'}[gap_read] # XXX: Add non-even reading!
				if cur_read - cur_off == 0:
					print '\t\t\t_databuf = br.Read%s();' % unit
				else:
					print '\t\t\t_databuf = (br.Read%s() << %i) | (_databuf >> %i);' % (unit, cur_read - cur_off, cur_off)
				assert type.bits <= gap_read + (cur_read - cur_off)
				cur_read += gap_read - cur_off
				cur_off = 0
				total_read += gap_read
				assert total_read <= total_bits
			type.unpack(name, '(_databuf >> %i)' % cur_off if cur_off else '_databuf', '\t\t\t')
			cur_off += type.bits
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
		#for name, type in self.elems:
		#	type.pack(name, '\t\t\t')
		print '\t\t}'

		print
		print '\t\tpublic override string ToString() {'
		print '\t\t\tvar ret = "bitfield %s {\\n";' % self.name
		for i, (name, type) in enumerate(self.elems):
			dec = type.declare()
			if dec is None or not name[0].isupper() or name in self.suppress:
				continue

			ws = '\t\t\t'

			print ws + 'ret += "\\t%s = ";' % name
			print ws + 'try {'
			oldws = ws
			ws += '\t'
			if type.base != 'string' and type.rank is not None:
				print ws + 'ret += "{\\n";'
				print ws + 'for(int i = 0, e = %s.%s; i < e; ++i)' % (name, 'Count' if type.base == 'list' else 'Length')
				print ws + '\tret += $"\\t\\t{ Indentify(%s[i], 2) }" + (i != e - 1 ? "," : "") + "\\n";' % name
				print ws + 'ret += "\\t}%s\\n";' % (',' if i != len(self.elems) - 1 else '')
			else:
				print ws + 'ret += $"{ Indentify(%s) }%s\\n";' % (name, ',' if i != len(self.elems) - 1 else '')
			ws = oldws
			print ws + '} catch(NullReferenceException) {'
			print ws + '\tret += "!!NULL!!\\n";'
			print ws + '}'

		print '\t\t\treturn ret + "}";'
		print '\t\t}'

		print '\t}'


class Enum(object):
	def __init__(self, name, ydef):
		type = Type(self, name)
		self.base = (' : ' + type.gen.declare()) if type.gen and type.gen.declare() != 'uint' else ''
		self.mbase = btypemap[type.gen.base if type.gen else 'uint32']
		self.cast = type.gen.declare() if type.gen else 'uint'
		self.name = type.base

		self.elems = []
		for elem in ydef:
			if isinstance(elem, dict):
				(name, values), = elem.items()
				self.elems.append((name, [values] if isinstance(values, int) else [x.strip() for x in values.split(',')]))
			else:
				self.elems.append((elem, []))

	def declare(self):
		print '\tpublic enum %s%s {' % (self.name, self.base)
		for i, (name, values) in enumerate(self.elems):
			print '\t\t%s%s%s' % (name, ' = %s' % values[0] if len(values) else '', ', ' if i != len(self.elems) - 1 else '')
		print '\t}'
		print '\tinternal static class %s_Helper {' % (self.name)
		print '\t\tinternal static %s Unpack(this %s val, BinaryReader br) {' % (self.name, self.name)
		print '\t\t\tswitch(br.Read%s()) {' % self.mbase
		for i, (name, values) in enumerate(self.elems):
			if i == len(self.elems) - 1:
				print '\t\t\t\tdefault:'
			else:
				print '\t\t\t\t%s' % ' '.join('case %s:' % value for value in values)
			print '\t\t\t\t\treturn %s.%s;' % (self.name, name)
		print '\t\t\t}'
		print '\t\t}'
		print '\t}'

	def pack(self):
		pass

	def unpack(self):
		pass
	pass

if __name__ == "__main__":
	sfile = yaml.load(file('structs.yml'), Loader=yaml.SafeLoader)

	sdefs = {top : (
		{name : Struct(name, struct) for name, struct in d['structs'].items()} if 'structs' in d else {}, 
		{name : Bitfield(name, bf) for name, bf in d['bitfields'].items()} if 'bitfields' in d else {}, 
		{name : Enum(name, enum) for name, enum in d['enums'].items()} if 'enums' in d else {}, 
		{name : value for name, value in d['constants'].items()} if 'constants' in d else {}
	) for top, d in sfile.items()}
	allEnums = {enum.name : enum for ns, (structs, bitfields, enums, constants) in sdefs.items() for name, enum in enums.items()}

	nsfiles = dict(
		login='LoginPackets.cs', 
		world='WorldPackets.cs', 
		zone='ZonePackets.cs', 
	)

	for ns, (structs, bitfields, enums, constants) in sdefs.items():
		with file(nsfiles[ns], 'w') as fp:
			sys.stdout = fp
			print r'''   /*
	*       o__ __o       o__ __o__/_   o          o    o__ __o__/_   o__ __o                o    ____o__ __o____   o__ __o__/_   o__ __o      
	*      /v     v\     <|    v       <|\        <|>  <|    v       <|     v\              <|>    /   \   /   \   <|    v       <|     v\     
	*     />       <\    < >           /  \o      / \  < >           / \     <\             / \         \o/        < >           / \     <\    
	*   o/                |            \o/ v\     \o/   |            \o/     o/           o/   \o        |          |            \o/       \o  
	*  <|       _\__o__   o__/_         |   <\     |    o__/_         |__  _<|           <|__ __|>      < >         o__/_         |         |> 
	*   \           |     |            / \    \o  / \   |             |       \          /       \       |          |            / \       //  
	*     \         /    <o>           \o/     v\ \o/  <o>           <o>       \o      o/         \o     o         <o>           \o/      /    
	*      o       o      |             |       <\ |    |             |         v\    /v           v\   <|          |             |      o     
	*      <\__ __/>     / \  _\o__/_  / \        < \  / \  _\o__/_  / \         <\  />             <\  / \        / \  _\o__/_  / \  __/>     
	*
	* THIS FILE IS GENERATED BY structgen.py/structs.yml
	* DO NOT EDIT
	*
	*/
'''
			print 'using System;'
			print 'using System.Collections.Generic;'
			print 'using System.IO;'
			print 'using static OpenEQ.Netcode.Utility;'

			if len(constants):
				print
				print 'using static OpenEQ.Netcode.%sConstants.Constants;' % ns.title()
				print 'namespace OpenEQ.Netcode.%sConstants {' % ns.title()
				print '\tinternal static class Constants {'
				for name, value in constants.items():
					print '\t\tpublic const int %s = %i;' % (name, value)
				print '\t}'
				print '}'

			print
			print 'namespace OpenEQ.Netcode {'

			if len(enums):
				for i, (name, enum) in enumerate(enums.items()):
					enum.declare()
					if i != len(enums) - 1:
						print
			if len(enums) and len(bitfields):
				print

			for i, (name, bitfield) in enumerate(bitfields.items()):
				bitfield.declare()
				if i != len(bitfields) - 1:
					print

			if (len(bitfields) and len(structs)) or (len(bitfields) == 0 and len(enums) and len(structs)):
				print

			for i, (name, struct) in enumerate(structs.items()):
				struct.declare()
				if i != len(structs) - 1:
					print
			print '}'

	pass

