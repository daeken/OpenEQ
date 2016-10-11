from buffer import Buffer
from zonefile import *

def readMod(data, zone, s3d):
    def getString(pos):
        return strs[pos:].split('\0', 1)[0]
    
    b = Buffer(data)
    assert b.read(4) == 'EQGM'

    version, strlen, nummat, numvert, numtri, unk = b.uint(6)
    assert version == 2
    strs = b.read(strlen)
    
    materials = {}
    for i in xrange(nummat):
        index = b.uint()
        matname, shader = getString(b.uint()), getString(b.uint())
        numprop = b.uint()
        materials[index] = matname, shader, {}
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
    
    vertices = b.float(numvert * 8)
    polygons = [(b.uint(3), b.uint(), b.uint()) for i in xrange(numtri)]

    print '%x' % b.pos, '%x' % len(b), numani
    print b.uint()
    assert b.pos == len(b)
