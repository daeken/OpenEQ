from cStringIO import StringIO
from struct import pack, unpack

class Buffer(object):
    def __init__(self, fp):
        if not hasattr(fp, 'read'):
            self.fp = StringIO(fp)
        else:
            self.fp = fp
    
    @property
    def pos(self):
        return self.fp.tell()
    @pos.setter
    def pos(self, pos):
        self.fp.seek(pos)
    
    def __len__(self):
        temp = self.pos
        self.fp.seek(0, 2)
        ret = self.pos
        self.pos = temp
        return ret
    
    def __iadd__(self, o):
        self.pos += o
        return self
    def __isub__(self, o):
        self.pos -= o
        return self
    
    def read(self, count):
        return self.fp.read(count)
    
    def uint(self, count=None):
        if count is None:
            return unpack('<I', self.read(4))[0]
        return unpack('<' + 'I'*count, self.read(4 * count))
    def int(self, count=None):
        if count is None:
            return unpack('<i', self.read(4))[0]
        return unpack('<' + 'i'*count, self.read(4 * count))
    def ushort(self, count=None):
        if count is None:
            return unpack('<H', self.read(2))[0]
        return unpack('<' + 'H'*count, self.read(2 * count))
    def short(self, count=None):
        if count is None:
            return unpack('<h', self.read(2))[0]
        return unpack('<' + 'h'*count, self.read(2 * count))
    def uchar(self, count=None):
        if count is None:
            return unpack('<B', self.read(1))[0]
        return unpack('<' + 'B'*count, self.read(count))
    def char(self, count=None):
        if count is None:
            return unpack('<b', self.read(1))[0]
        return unpack('<' + 'b'*count, self.read(count))
    def float(self, count=None):
        if count is None:
            return unpack('<f', self.read(4))[0]
        return unpack('<' + 'f'*count, self.read(4 * count))
    def vec3(self, count=None):
        if count is None:
            return unpack('<fff', self.read(12))
        return tuple(unpack('<fff', self.read(12)) for i in xrange(count))
