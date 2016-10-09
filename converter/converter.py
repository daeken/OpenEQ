from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import readWld
from zonefile import *

def convertOld(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        zone = Zone()
        files = readS3D(file('eqdata/%s.s3d' % name, 'rb'))
        readWld(files['%s.wld' % name], zone, files, isZone=True)
        zone.output(zip)

def main(name):
    convertOld(name)

if __name__=='__main__':
    import sys
    main(*sys.argv[1:])
