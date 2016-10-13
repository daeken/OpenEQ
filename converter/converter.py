from os.path import isfile
from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import Wld
from zon import readZon
from zonefile import *

def s3dFallback(*filedicts):
    ndicts = []
    for files in filedicts:
        nfiles = {}
        ndicts.append(nfiles)
        for xfiles in filedicts:
            if xfiles is not files:
                nfiles.update(xfiles)
        nfiles.update(files)
    return ndicts

def convertOld(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        ofiles = readS3D(file('eqdata/%s_obj.s3d' % name, 'rb'))
        zfiles = readS3D(file('eqdata/%s.s3d' % name, 'rb'))
        ofiles, zfiles = s3dFallback(ofiles, zfiles)

        Wld(ofiles['%s_obj.wld' % name], ofiles).convertObjects(zone)
        Wld(zfiles['objects.wld'], zfiles).convertObjects(zone)
        Wld(zfiles['%s.wld' % name], zfiles).convertZone(zone)
        zone.output(zip)

def convertChr(name):
    pass

def convertNew(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        zfiles = readS3D(file('eqdata/%s.eqg' % name, 'rb'))
        #for fn, data in zfiles.items():
        #    file('s3data/%s' % fn, 'wb').write(data)
        if '%s.zon' % name in zfiles:
            readZon(zfiles['%s.zon' % name], zone, zfiles)
        else:
            readZon(file('eqdata/%s.zon' % name, 'rb').read(), zone, zfiles)
        zone.output(zip)

def main(name):
    if '_chr' in name:
        convertChr(name)
    elif isfile('eqdata/%s.s3d' % name):
        convertOld(name)
    elif isfile('eqdata/%s.eqg' % name):
        convertNew(name)
    else:
        print 'Cannot find zone'
        return
    print 'All Done'

if __name__=='__main__':
    import sys
    main(*sys.argv[1:])
