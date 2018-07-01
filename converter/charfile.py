import struct, tempfile
from zonefile import resampleTexture

class Charfile(object):
	def __init__(self, name):
		self.name = name
		self.meshes = []
		self.animations = {}
		self.boneparents = {}

	def addMesh(self, mesh):
		self.meshes.append(mesh)

	def addBoneParent(self, child, parent):
		self.boneparents[child] = parent

	def addAnimation(self, aniname, boneframes):
		self.animations[aniname] = boneframes

	def out(self, zip):
		assets = {}
		for mesh in self.meshes:
			for i, filename in enumerate(mesh.material.filenames):
				if filename not in assets:
					assets[filename] = mesh.material.textures[i]
		for k, v in assets.items():
			v = resampleTexture(v)
			zip.writestr(k, v)

		def oint(*x):
			zout.write(struct.pack('<' + 'i'*len(x), *x))
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
			ouint(len(self.meshes))
			for mesh in self.meshes:
				ouint(mesh.material.flags)
				ouint(len(mesh.material.filenames))
				for filename in mesh.material.filenames:
					ostring(filename)

				ouint(len(mesh.vertbuffer))
				data = mesh.vertbuffer.data
				positions = []
				normals = []
				texcoords = []
				bones = []
				for i in xrange(len(mesh.vertbuffer)):
					positions += data[i * 9 + 0 + 0:i * 9 + 0 + 3]
					normals += data[i * 9 + 3 + 0:i * 9 + 3 + 3]
					texcoords += data[i * 9 + 6 + 0:i * 9 + 6 + 2]
					bones.append(data[i * 9 + 8])
				ofloat(*positions)
				ofloat(*normals)
				ofloat(*texcoords)
				ouint(*bones)

				ouint(len(mesh.polygons))
				for poly in mesh.polygons:
					ouint(*poly)

			ouint(len(self.boneparents))
			for i in xrange(len(self.boneparents)):
				oint(self.boneparents[i])

			ouint(len(self.animations))
			for name, boneframes in self.animations.items():
				print name
				ostring(name)
				for data in boneframes:
					ouint(len(data) / 7)
					ofloat(*data)

			zout.seek(0)
			zip.writestr('%s.oec' % self.name, zout.read())
