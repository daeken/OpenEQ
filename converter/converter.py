from zipfile import ZipFile

from buffer import Buffer
from s3d import readS3D
from wld import readWld

def convertOld(name):
    with ZipFile('%s.zip' % name, 'w') as zip:
        files = readS3D(file('eqdata/%s.s3d' % name, 'rb'))
        zip.writestr('zone.bin', readWld(files['%s.wld' % name]))

def main(name):
    convertOld(name)

if __name__=='__main__':
    import sys
    main(*sys.argv[1:])
