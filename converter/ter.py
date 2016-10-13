from buffer import Buffer
from zonefile import *

def readTer(data, zone, s3d):
    def getString(pos):
        return strs[pos:].split('\0', 1)[0]
    
    b = Buffer(data)
    assert b.read(4) == 'EQGT'

    version, strlen, nummat, numvert, numtri = b.uint(5)
    strs = b.read(strlen)

    materials = {}
    for i in xrange(nummat):
        index = b.uint()
        matname, shader = getString(b.uint()), getString(b.uint())
        numprop = b.uint()
        materials[i] = matname, shader, {}
        for j in xrange(numprop):
            pname = getString(b.uint())
            type = b.uint()
            if type == 0:
                value = b.float()
            elif type == 2:
                value = getString(b.uint())
            elif type == 3:
                value = b.uint()
            else:
                print 'unknown type:', type
                return
            materials[index][2][pname] = value
    
    if version == 3:
        vertices = []
        for i in xrange(numvert):
            vertices += b.float(6)
            b += 12 # ignore 3 floats
            vertices += b.float(2)
    else:
        vertices = b.float(numvert * 8)
    polygons = [(b.uint(3), b.uint(), b.uint()) for i in xrange(numtri)]

    assert (version == 3 or b.uint() == 0) and b.pos == len(b)

    invisible = Material(FLAG_TRANSPARENT, [])
    zmats = {0xFFFFFFFF : invisible}
    for index, (name, shader, props) in materials.items():
        visible = 'e_TextureDiffuse0' in props
        zmats[index] = Material(FLAG_NORMAL if visible else FLAG_TRANSPARENT, [s3d[props['e_TextureDiffuse0'].lower()]] if visible else [])

    matpolys = {k : [] for k in zmats}
    for (c, b, a), matid, flags in polygons:
        #if flags == 0x00050000:
        matpolys[matid].append((flags != 0, (a, b, c)))
    # 0x0, 0x00010000, 0x00020000, 0x00040000 -- normal?
    # 0x1, 0x2 -- invisible
    # 0x00030000, 0x00050000 -- non-collidable?
    # '00000000', '00000001', '00000002', '00010000', '00020000', '00030000', '00040000', 
    # '00050000', '00080000', '00090000', '000a0000', '00100000', '00200000', '00400000', 
    # '00600000', '00800000', '01000000', '02000000', '04000000', '08000000', '10000000', 
    # '20000000', '40000000', '80000000'
    
    for index, polys in matpolys.items():
        zone.zoneobj.addMesh(Mesh(zmats[index], VertexBuffer(vertices, numvert), polys))
