# This is genuinely some of the ugliest code I've ever written.
# Please send help.

from buffer import Buffer
from utility import *
from zonefile import *

fragHandlers = {}
def fragment(type):
    def sub(f):
        fragHandlers[type] = f
        return f
    return sub

@fragment(0x03)
def frag_texname(b, ref):
    size = b.uint() + 1
    return [decodeString(b.read(b.ushort()))[:-1] for i in xrange(size)]

@fragment(0x04)
def frag_texbitinfo(b, ref):
    flags = b.uint()
    size = b.uint()
    if flags & (1 << 2):
        b.uint()
    if flags & (1 << 3):
        b.uint()
    return map(ref, b.int(size))

@fragment(0x05)
def frag_texunk(b, ref):
    return ref(b.int())

FLAG_NORMAL = 0
FLAG_MASKED = 1 << 1
FLAG_TRANSLUCENT = 1 << 2
FLAG_TRANSPARENT = 1 << 3

@fragment(0x30)
def frag_texref(b, ref):
    pairflags = b.uint()
    flags = b.uint()
    b += 12
    if (pairflags & 2) == 2:
        b += 8
    saneflags = 0
    if flags == 0:
        saneflags = FLAG_TRANSPARENT
    if (flags & (2 | 8 | 16)) != 0:
        saneflags |= FLAG_MASKED
    if (flags & (4 | 8)) != 0:
        saneflags |= FLAG_TRANSLUCENT

    return saneflags, ref(b.int())

@fragment(0x31)
def frag_texlist(b, ref):
    b += 4
    return map(ref, b.int(b.uint()))

@fragment(0x36)
def frag_mesh(b, ref):
    out = {}

    flags, tlistref, aniref = b.uint(3)
    out['textures'] = ref(tlistref)
    b += 8
    center = b.vec3()
    b += 12
    maxdist = b.float()
    min, max = b.vec3(2)
    vertcount, texcoordcount, normalcount, colorcount = b.ushort(4)
    polycount, vertpiececount, polytexcount, verttexcount = b.ushort(4)
    size9 = b.ushort()
    scale = float(1 << b.ushort())

    out['vertices'] = [(b.short() / scale + center[0], b.short() / scale + center[1], b.short() / scale + center[2]) for i in xrange(vertcount)]
    if texcoordcount == 0:
        out['texcoords'] = [(0, 0)] * vertcount
    else:
        out['texcoords'] = [b.ushort(2) if old else b.uint(2) for i in xrange(texcoordcount)]
    out['normals'] = [(b.char() / 127., b.char() / 127., b.char() / 127.) for i in xrange(normalcount)]
    out['colors'] = b.uint(colorcount)
    out['polys'] = [(b.ushort() != 0x0010, b.ushort(3)) for i in xrange(polycount)]
    b += 4 * vertpiececount
    out['polytex'] = [b.ushort(2) for i in xrange(polytexcount)]

    return out

xorkey = 0x95, 0x3A, 0xC5, 0x2A, 0x95, 0x7A, 0x95, 0x6A
def decodeString(s):
    return ''.join(chr(ord(x) ^ xorkey[i % len(xorkey)]) for i, x in enumerate(s))
def readWld(data, zone, s3d, isZone):
    global old
    class FragRef(object):
        def __init__(self, id=None, name=None):
            self.id = id
            self.name = name
        def resolve(self):
            if self.id is not None:
                return frags[self.id][3]
            return names[self.name][3]
        def __repr__(self):
            if self.id is not None:
                return 'FragRef(id=%r)' % self.id
            return 'FragRef(name=%r)' % self.name
    def dereference(val):
        if isinstance(val, dict):
            return {k : dereference(v) for k, v in val.items()}
        elif isinstance(val, tuple) or isinstance(val, list):
            return map(dereference, val)
        elif isinstance(val, FragRef):
            return val.resolve()
        return val
    
    def getString(i):
        return stringTable[i:].split('\0', 1)[0]
    def getFrag(ref):
        if ref > 0:
            ref -= 1
            if ref in frags:
                return frags[ref][3]
            return FragRef(id=ref)
        name = getString(-ref)
        if name in names:
            return names[name][3]
        return FragRef(name=name)

    b = Buffer(data)

    assert b.uint() == 0x54503D02
    old = b.uint() == 0x00015500
    fragCount = b.uint()
    b += 8
    hashlen = b.uint()
    b += 4
    stringTable = decodeString(b.read(hashlen))
    
    frags = {}
    names = {}
    byType = {}
    for i in xrange(fragCount):
        size = b.uint()
        type = b.uint()
        nameoff = b.int()
        name = getString(-nameoff) if nameoff != 0x1000000 else None
        epos = b.pos + size - 4
        frag = fragHandlers[type](b, getFrag) if type in fragHandlers else None
        if isinstance(frag, dict):
            frag['_name'] = name
        frag = (i, name, type, frag)
        names[name] = frags[i] = frag
        if type not in byType:
            byType[type] = []
        byType[type].append(frag)
        b.pos = epos
    
    byType = {k : [] for k in byType}
    nfrags = {}
    nnames = {}
    for i, name, type, frag in frags.values():
        frag = dereference(frag)
        nfrags[i] = nnames[name] = frag
        byType[type].append(frag)
    frags = nfrags
    names = nnames
    
    if isZone:
        print 'building zone ...'
        for meshfrag in byType[0x36]:
            vbuf = VertexBuffer(flatten(interleave(meshfrag['vertices'], meshfrag['normals'], meshfrag['texcoords'])), len(meshfrag['vertices']))

            off = 0
            for count, index in meshfrag['polytex']:
                texflags, ((texname, ), ) = meshfrag['textures'][index] 
                material = Material(texflags, s3d[texname.lower()])
                mesh = Mesh(material, vbuf, meshfrag['polys'][off:off+count])
                zone.zoneobj.addMesh(mesh)
                off += count
