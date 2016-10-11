from os.path import isfile
from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import readWld
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

        readWld(ofiles['%s_obj.wld' % name], zone, ofiles, isZone=False)
        readWld(zfiles['objects.wld'], zone, zfiles, isZone=False)
        readWld(zfiles['%s.wld' % name], zone, zfiles, isZone=True)
        zone.output(zip)

def convertNew(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        zfiles = readS3D(file('eqdata/%s.eqg' % name, 'rb'))
        #for fn, data in zfiles.items():
        #    file('s3data/%s' % fn, 'wb').write(data)
        readZon(zfiles['%s.zon' % name], zone, zfiles)
        zone.output(zip)

def main(name):
    if isfile('eqdata/%s.s3d' % name):
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
