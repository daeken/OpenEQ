import math
from buffer import Buffer
from utility import *
from zonefile import *

class FragRef(object):
    def __init__(self, wld, id=None, name=None, value=None):
        self.wld = wld
        self.id = id
        self.name = name
        self.value = value
    def resolve(self):
        if self.value is not None:
            return self.value
        if self.id is not None:
            self.value = self.wld.frags[self.id][3]
        elif self.name is not None:
            self.value = self.wld.names[self.name][3]
        if isinstance(self.value, FragRef):
            self.value = self.value.resolve()
        return self.value
    def __getitem__(self, index):
        return self.resolve()[index]
    def __len__(self):
        return len(self.resolve())
    def __repr__(self):
        if self.id is not None:
            return 'FragRef(id=%r)' % self.id
        elif self.name is not None:
            return 'FragRef(name=%r)' % self.name
        return 'FragRef(value=%r)' % self.value

fragHandlers = {}
def fragment(type):
    def sub(f):
        fragHandlers[type] = f
        return f
    return sub

class Wld(object):
    def __init__(self, data, s3d):
        self.b = b = Buffer(data)
        self.s3d = s3d

        assert b.uint() == 0x54503D02
        self.old = b.uint() == 0x00015500
        fragCount = b.uint()
        b += 8
        hashlen = b.uint()
        b += 4
        self.stringTable = self.decodeString(b.read(hashlen))
        
        self.frags = {}
        self.names = {}
        self.byType = {}
        for i in xrange(fragCount):
            size = b.uint()
            type = b.uint()
            nameoff = b.int()
            name = self.getString(-nameoff) if nameoff != 0x1000000 else None
            epos = b.pos + size - 4
            frag = fragHandlers[type](self) if type in fragHandlers else None
            if isinstance(frag, dict):
                frag['_name'] = name
            frag = (i, name, type, frag)
            self.names[name] = self.frags[i] = frag
            if type not in self.byType:
                self.byType[type] = []
            self.byType[type].append(frag)
            b.pos = epos
        
        self.byType = {k : [] for k in self.byType}
        nfrags = {}
        nnames = {}
        for i, name, type, frag in self.frags.values():
            nfrags[i] = nnames[name] = frag
            self.byType[type].append(frag)
        self.frags = nfrags
        self.names = nnames

        print 'fragtypes:', ['%02x' % x for x in self.byType.keys()]
    
    def convertZone(self, zone):
        for meshfrag in self.byType[0x36]:
            vbuf = VertexBuffer(flatten(interleave(meshfrag['vertices'], meshfrag['normals'], meshfrag['texcoords'])), len(meshfrag['vertices']))

            off = 0
            for count, index in meshfrag['polytex']:
                texflags, mtex = meshfrag['textures'][index]
                texnames = [x[0] for x in mtex]
                material = Material(texflags, [self.s3d[texname.lower()] for texname in texnames])
                mesh = Mesh(material, vbuf, meshfrag['polys'][off:off+count])
                zone.zoneobj.addMesh(mesh)
                off += count
    
    def convertObjects(self, zone):
        if 0x36 in self.byType:
            for meshfrag in self.byType[0x36]:
                obj = zone.addObject(meshfrag['_name'].replace('_DMSPRITEDEF', ''))
                vbuf = VertexBuffer(flatten(interleave(meshfrag['vertices'], meshfrag['normals'], meshfrag['texcoords'])), len(meshfrag['vertices']))

                off = 0
                for count, index in meshfrag['polytex']:
                    texflags, mtex = meshfrag['textures'][index]
                    texnames = [x[0] for x in mtex]
                    material = Material(texflags, [self.s3d[texname.lower()] for texname in texnames])
                    mesh = Mesh(material, vbuf, meshfrag['polys'][off:off+count])
                    obj.addMesh(mesh)
                    off += count
        if 0x15 in self.byType:
            for objfrag in self.byType[0x15]:
                objname = self.getString(-objfrag['nameoff']).replace('_ACTORDEF', '')
                zone.addPlaceable(objname, objfrag['position'], objfrag['rotation'], objfrag['scale'])
    
    def getString(self, i):
        return self.stringTable[i:].split('\0', 1)[0]
    
    xorkey = 0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A
    def decodeString(self, s):
        return ''.join(chr(ord(x) ^ Wld.xorkey[i % len(Wld.xorkey)]) for i, x in enumerate(s))
    
    def getFrag(self, ref):
        if ref > 0:
            ref -= 1
            if ref in self.frags:
                return FragRef(self, value=self.frags[ref][3])
            return FragRef(self, id=ref)
        name = self.getString(-ref)
        if name in self.names:
            return FragRef(self, value=self.names[name][3])
        return FragRef(self, name=name)
    
    @fragment(0x03)
    def frag_texname(self):
        size = self.b.uint() + 1
        return [self.decodeString(self.b.read(self.b.ushort()))[:-1] for i in xrange(size)]

    @fragment(0x04)
    def frag_texbitinfo(self):
        flags = self.b.uint()
        size = self.b.uint()
        if flags & (1 << 2):
            self.b.uint()
        if flags & (1 << 3):
            self.b.uint()
        return map(self.getFrag, self.b.int(size))

    @fragment(0x05)
    def frag_texunk(self):
        return self.getFrag(self.b.int())

    @fragment(0x15)
    def frag_objloc(self):
        sref = self.b.int()
        flags = self.b.uint()
        frag1_unk = self.b.uint()
        pos = self.b.float(3)
        rot = self.b.float(3)
        rot = (rot[2] / 512. * 360. * math.pi / 180., rot[1] / 512. * 360. * math.pi / 180., rot[0] / 512. * 360. * math.pi / 180.)
        scale = self.b.float(3)
        scale = (scale[2], scale[2], scale[2]) if scale[2] > 0.0001 else (1, 1, 1)
        frag2_unk = self.b.uint()
        params2 = self.b.uint()

        return dict(position=pos, rotation=rot, scale=scale, nameoff=sref)

    @fragment(0x30)
    def frag_texref(self):
        pairflags = self.b.uint()
        flags = self.b.uint()
        self.b += 12
        if (pairflags & 2) == 2:
            self.b += 8
        saneflags = 0
        if flags == 0:
            saneflags = FLAG_TRANSPARENT
        if (flags & (2 | 8 | 16)) != 0:
            saneflags |= FLAG_MASKED
        if (flags & (4 | 8)) != 0:
            saneflags |= FLAG_TRANSLUCENT

        return saneflags, self.getFrag(self.b.int())

    @fragment(0x31)
    def frag_texlist(self):
        self.b += 4
        return map(self.getFrag, self.b.int(self.b.uint()))

    @fragment(0x36)
    def frag_mesh(self):
        out = {}

        flags, tlistref, aniref = self.b.uint(3)
        out['textures'] = self.getFrag(tlistref)
        self.b += 8
        center = self.b.vec3()
        self.b += 12
        maxdist = self.b.float()
        min, max = self.b.vec3(2)
        vertcount, texcoordcount, normalcount, colorcount = self.b.ushort(4)
        polycount, vertpiececount, polytexcount, verttexcount = self.b.ushort(4)
        size9 = self.b.ushort()
        scale = float(1 << self.b.ushort())

        out['vertices'] = [(self.b.short() / scale + center[0], self.b.short() / scale + center[1], self.b.short() / scale + center[2]) for i in xrange(vertcount)]
        if texcoordcount == 0:
            out['texcoords'] = [(0, 0)] * vertcount
        else:
            out['texcoords'] = [(self.b.short() / 256., self.b.short() / 256.) if self.old else self.b.float(2) for i in xrange(texcoordcount)]
        out['normals'] = [(self.b.char() / 127., self.b.char() / 127., self.b.char() / 127.) for i in xrange(normalcount)]
        out['colors'] = self.b.uint(colorcount)
        out['polys'] = [(self.b.ushort() != 0x0010, self.b.ushort(3)) for i in xrange(polycount)]
        self.b += 4 * vertpiececount
        out['polytex'] = [self.b.ushort(2) for i in xrange(polytexcount)]

        return out
