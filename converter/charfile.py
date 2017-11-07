import struct, tempfile

class Charfile(object):
	def __init__(self, name):
		self.name = name
		self.matpolys = []
		self.animations = {}

	def addMaterial(self, material, polys):
		self.matpolys.append((material, polys))

	def addFrame(self, aniname, vertbuffer):
		if aniname not in self.animations:
			self.animations[aniname] = []

		self.animations[aniname].append(vertbuffer)

	def out(self, zip):
		assets = {}
		for material, _ in self.matpolys:
			for i, filename in enumerate(material.filenames):
				if filename not in assets:
					assets[filename] = material.textures[i]
		for k, v in assets.items():
			zip.writestr(k, v)

		def ouint(*x):
			zout.write(struct.pack('<' + 'I'*len(x), *x))
		def ofloat(*x):
			zout.write(struct.pack('<' + 'f'*len(x), *x))
		def ostring(x):
			sl = len(x)
			if sl == 0:
				zout.write(chr(0))
			while sl:
				zout.write(chr((sl & 0x7F) | (0x80 if sl > 127 else 0x00)))
				sl >>= 7
			zout.write(x)
		with tempfile.TemporaryFile() as zout:
			ouint(len(self.matpolys))
			for material, polys in self.matpolys:
				ouint(material.flags)
				ouint(len(material.filenames))
				for filename in material.filenames:
					ostring(filename)
				ouint(len(polys) / 3)
				ouint(*polys)

			ouint(len(self.animations[self.animations.keys()[0]][0]) / 8) # Number of vertices
			ouint(len(self.animations))
			for name, frames in self.animations.items():
				ostring(name)
				ouint(len(frames))
				for frame in frames:
					ofloat(*frame)

			zout.seek(0)
			zip.writestr('%s.oec' % self.name, zout.read())
