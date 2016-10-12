#!/usr/bin/env python

import argparse, os, s3d, sys

def main():
    parser = argparse.ArgumentParser(description='Decompress S3D/EQG files')
    parser.add_argument('file', type=unicode, help='file to decompress')
    parser.add_argument('-o', '--outdir', nargs='?', dest='outdir', help='output directory (default: make a new subdirectory with the same name)')
    parser.add_argument('-v', dest='verbose', action='store_true', help='show all files written')

    args = parser.parse_args()

    try:
        files = s3d.readS3D(file(args.file))
    except:
        print >>sys.stderr, 'Could not open file:', args.file
        return 1
    
    if args.outdir is None:
        args.outdir = '_'.join(args.file.rsplit('.', 1))
    
    try:
        os.mkdir(args.outdir)
    except:
        pass

    for fn, data in files.items():
        if args.verbose:
            print fn
        try:
            with file('%s/%s' % (args.outdir, fn), 'wb') as fp:
                fp.write(data)
        except:
            print >>sys.stderr, 'Could not open file for writing:', '%s/%s' % (args.outdir, fn)
            return 1
    
    return 0

if __name__=='__main__':
    sys.exit(main())
