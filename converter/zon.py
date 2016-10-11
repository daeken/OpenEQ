from buffer import Buffer
from ter import readTer
from mod import readMod

def readZon(data, zone, s3d):
    def getString(pos):
        return strs[pos:].split('\0', 1)[0]

    b = Buffer(data)
    assert b.read(4) == 'EQGZ'

    flags, strlen = b.uint(2)
    numfiles, numplaceable, numunk3 = b.uint(3)
    numlights = b.uint()

    strs = b.read(strlen)

    files = [getString(b.uint()).lower() for i in xrange(numfiles)]
    for fn in files:
        if fn.endswith('.ter'):
            readTer(s3d[fn], zone, s3d)
        elif fn.endswith('.mod'):
            print 'reading mod', fn
            readMod(s3d[fn], zone, s3d)
    
    for i in xrange(numplaceable):
        fileid = files[b.uint()]
        name = getString(b.uint())
        pos = b.float(3)
        rot = b.float(3)
        scale = b.float()
        #print fileid, name, pos, rot, scale
    
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
