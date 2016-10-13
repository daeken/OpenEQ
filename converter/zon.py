from buffer import Buffer
from ter import readTer
from mod import readMod
from zonefile import *

def readZon(data, zone, s3d):
    def getString(pos):
        return strs[pos:].split('\0', 1)[0]

    b = Buffer(data)
    assert b.read(4) == 'EQGZ'

    version, strlen = b.uint(2)
    numfiles, numplaceable, numunk3 = b.uint(3)
    numlights = b.uint()

    strs = b.read(strlen)

    files = [getString(b.uint()).lower() for i in xrange(numfiles)]
    objects = []
    for fn in files:
        if fn.endswith('.ter'):
            readTer(s3d[fn], zone, s3d)
            objects.append(None) # Shouldn't be placing zones...
        elif fn.endswith('.mod'):
            print 'reading mod', fn
            obj = zone.addObject(name=fn)
            #readMod(s3d[fn], obj, s3d)
            objects.append(obj)
    
    for i in xrange(numplaceable):
        #objname = files[b.uint()]
        objname = ''
        name = getString(b.uint())
        pos = b.float(3)
        ra, rb, rc = b.float(3)
        rot = (rc, rb, ra)
        scale = b.float()

        #zone.addPlaceable(objname, pos, rot, (scale, scale, scale))
    
    for i in xrange(numunk3):
        u1 = b.uint()
        u2 = b.float(9)
        #print getString(u1), u2
    
    #b.pos = len(b) - (32 * numplaceable)
    for i in xrange(numlights):
        obj = strs[b.uint():].split('\0', 1)[0]
        pos = b.float(3)
        rot = b.float(3)
        scale = b.float()
        #print obj, pos, rot, scale
