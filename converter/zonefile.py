import hashlib, struct, tempfile
from pyrr import Quaternion, Vector3
import numpy as np

resample = True
if resample:
    from PIL import Image
    from cStringIO import StringIO
    def resampleTexture(data):
        fp = StringIO(data)
        im = Image.open(fp)
        #if im.width != im.height:
        #im = im.transpose(Image.FLIP_LEFT_RIGHT)
        #print im.width, im.height, im.mode
        return struct.pack('<II', im.width, im.height) + im.tobytes()

class VertexBuffer(object):
    def __init__(self, data, count):
        self.data = data
        self.count = count
        assert len(data) % count == 0
        self.stride = len(data) / count
    
    def __getitem__(self, index):
        return self.data[index * self.stride:(index + 1) * self.stride]
    
    def __len__(self):
        return self.count

FLAG_NORMAL = 0
FLAG_MASKED = 1 << 0
FLAG_TRANSLUCENT = 1 << 1
FLAG_TRANSPARENT = 1 << 2

class Material(object):
    def __init__(self, flags, textures, param):
        self.flags = flags
        self.filenames = tuple(hashlib.sha256(data).hexdigest() + '.dds' for data in textures) 
        self.textures = textures
        self.param = param

class Mesh(object):
    def __init__(self, material, vertbuffer, polygons, collidable=True):
        self.material = material
        self.vertbuffer = vertbuffer
        self.polygons = polygons
        self.collidable = collidable
    
    def add(self, mesh):
        offset = self.vertbuffer.count
        if self.vertbuffer is not mesh.vertbuffer:
            self.vertbuffer.data += mesh.vertbuffer.data
            self.vertbuffer.count += mesh.vertbuffer.count
            self.polygons += [(a+offset, b+offset, c+offset) for a, b, c in mesh.polygons]
        else:
            self.polygons += mesh.polygons
        return self
    
    def subset(self, cpoly):
        vbuffer = [[]]
        npoly = []
        used = {}
        
        def mapIndex(i):
            if i in used:
                return used[i]
            ind = used[i] = len(used)
            vbuffer[0] += self.vertbuffer[i]
            return ind
        
        for a, b, c in cpoly:
            npoly.append((mapIndex(a), mapIndex(b), mapIndex(c)))
        
        return Mesh(self.material, VertexBuffer(vbuffer[0], len(used)), npoly, collidable=self.collidable)
    
    def optimize(self):
        def pushmesh():
            outmeshes.append(self.subset(cpoly))
        outmeshes = []
        cpoly, cverts = [], set()
        for a, b, c in self.polygons:
            na, nb, nc = a in cverts, b in cverts, c in cverts
            if na: cverts.add(a)
            if nb: cverts.add(b)
            if nc: cverts.add(c)
            cpoly.append((a, b, c))
        if len(cpoly):
            pushmesh()
        return outmeshes

class Object(object):
    def __init__(self, name=''):
        self.name = name
        self.meshes = []
    
    def addMesh(self, mesh):
        self.meshes.append(mesh)
        return self

class Placeable(object):
    def __init__(self, obj, position, rotation, scale):
        self.obj = obj
        self.position = position
        self.rotation = rotation
        self.scale = scale

class Zone(object):
    def __init__(self):
        self.objects = [Object()]
        self.objnames = {}
        self.zoneobj = self.objects[0]
        self.placeables = []
    
    def addObject(self, name):
        obj = Object(name)
        self.objects.append(obj)
        self.objnames[name] = obj
        return obj
    
    def addPlaceable(self, objname, position, rotation, scale):
        if objname not in self.objnames:
            print 'Could not place object %r' % objname
            return
        placeable = Placeable(self.objnames[objname], position, rotation, scale)
        self.placeables.append(placeable)
        return placeable
    
    def coalesceObjectMeshes(self):
        for obj in self.objects:
            startmeshcount = len(obj.meshes)
            matmeshes = {}
            for mesh in obj.meshes:
                mat = mesh.material.filenames, mesh.material.flags, mesh.collidable
                if mat not in matmeshes:
                    matmeshes[mat] = []
                matmeshes[mat].append(mesh)
            obj.meshes = []
            poss = 0
            for meshlist in matmeshes.values():
                if len(meshlist) == 1:
                    obj.meshes.append(meshlist[0])
                    continue
                gmesh = reduce(lambda a, b: a.add(b), meshlist)
                obj.meshes += gmesh.optimize()
    
    def output(self, zip):
        self.coalesceObjectMeshes()

        assets = {}
        for obj in self.objects:
            for mesh in obj.meshes:
                material = mesh.material
                for i, filename in enumerate(material.filenames):
                    if filename not in assets:
                        assets[filename] = material.textures[i]
        if resample:
            print 'Resampling textures'
        for k, v in assets.items():
            if resample:
                v = resampleTexture(v)
            zip.writestr(k, v)
        if resample:
            print 'Done'
        
        for obj in self.objects:
            obj.meshes = [x for m in obj.meshes for x in m.optimize()]
        
        def ouint(*x):
            zout.write(struct.pack('<' + 'I'*len(x), *x))
        def ofloat(*x):
            zout.write(struct.pack('<' + 'f'*len(x), *x))
        def rewind(data):
            out = []
            for i in xrange(0, len(data), 3):
                out += [data[i + 0], data[i + 2], data[i + 1]]
            return out
        rotation = Quaternion.from_x_rotation(-np.pi / 2)
        def rotate(data, xs=1, ys=1, zs=1):
            out = []
            if xs != 1 or ys != 1 or zs != 1:
                scale = Vector3([xs, ys, zs])
            else:
                scale = None
            for i in xrange(0, len(data), 3):
                vec = rotation * Vector3(data[i:i+3])
                if scale is not None:
                    vec *= scale
                out += list(vec)
            return out
        def ostring(x):
            sl = len(x)
            if sl == 0:
                zout.write(chr(0))
            while sl:
                zout.write(chr((sl & 0x7F) | (0x80 if sl > 127 else 0x00)))
                sl >>= 7
            zout.write(x)
        with tempfile.TemporaryFile() as zout:
            materials = {}
            for obj in self.objects:
                for mesh in obj.meshes:
                    mat = mesh.material.flags, mesh.material.param, mesh.material.filenames
                    if mat not in materials:
                        materials[mat] = len(materials)
            ouint(len(materials))
            for (flags, param, filenames), id in sorted(materials.items(), cmp=lambda a, b: cmp(a[1], b[1])):
                ouint(flags)
                ouint(param)
                ouint(len(filenames))
                for filename in filenames:
                    ostring(filename)
            ouint(len(self.objects))
            objrefs = {}
            for i, obj in enumerate(self.objects):
                objrefs[obj] = i
                ouint(len(obj.meshes))
                for mesh in obj.meshes:
                    matid = materials[(mesh.material.flags, mesh.material.param, mesh.material.filenames)]
                    ouint(matid)
                    ouint(1 if mesh.collidable else 0)
                    ouint(len(mesh.vertbuffer))
                    data = mesh.vertbuffer.data
                    positions = []
                    normals = []
                    texcoords = []
                    for i in xrange(len(mesh.vertbuffer)):
                        positions += data[i * 8 + 0 + 0:i * 8 + 0 + 3]
                        normals += data[i * 8 + 3 + 0:i * 8 + 3 + 3]
                        texcoords += data[i * 8 + 6 + 0:i * 8 + 6 + 2]
                    ofloat(*rotate(positions))
                    ofloat(*rotate(normals))
                    ofloat(*texcoords)
                    ouint(len(mesh.polygons))
                    for x in mesh.polygons:
                        ouint(*x)

            ouint(len(self.placeables))
            for placeable in self.placeables:
                ouint(objrefs[placeable.obj])
                ofloat(*rotate(placeable.position))
                ofloat(*rotate(placeable.rotation, ys=-1))
                ofloat(*rewind(placeable.scale))
            
            zout.seek(0)
            zip.writestr('zone.oez', zout.read())
