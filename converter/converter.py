from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import readWld
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

def main(name):
    convertOld(name)

if __name__=='__main__':
    import sys
    main(*sys.argv[1:])
