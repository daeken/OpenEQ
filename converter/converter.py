from os.path import isfile
from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import readWld
from zon import readZon
from zonefile import *

def convertOld(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        ofiles = readS3D(file('eqdata/%s_obj.s3d' % name, 'rb'))
        readWld(ofiles['%s_obj.wld' % name], zone, ofiles, isZone=False)
        zfiles = readS3D(file('eqdata/%s.s3d' % name, 'rb'))
        readWld(zfiles['objects.wld'], zone, zfiles, isZone=False)
        readWld(zfiles['%s.wld' % name], zone, zfiles, isZone=True)
        zone.output(zip)

def convertNew(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        zfiles = readS3D(file('eqdata/%s.eqg' % name, 'rb'))
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
