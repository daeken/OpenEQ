import hashlib, struct, tempfile

resample = False
if resample:
    from PIL import Image
    from cStringIO import StringIO
    def resampleTexture(data):
        fp = StringIO(data)
        im = Image.open(fp)
        outfp = StringIO()
        im.save(outfp, 'png')
        return outfp.getvalue()

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

class Material(object):
    def __init__(self, flags, textures):
        self.flags = flags
        self.filenames = tuple(hashlib.sha256(data).hexdigest() + '.dds' for data in textures) 
        self.textures = textures

class Mesh(object):
    def __init__(self, material, vertbuffer, polygons):
        self.material = material
        self.vertbuffer = vertbuffer
        self.polygons = polygons
    
    def add(self, mesh):
        offset = self.vertbuffer.count
        self.vertbuffer.data += mesh.vertbuffer.data
        self.vertbuffer.count += mesh.vertbuffer.count
        self.polygons += [(collidable, (a+offset, b+offset, c+offset)) for collidable, (a, b, c) in mesh.polygons]
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
        
        for collidable, (a, b, c) in cpoly:
            npoly.append((collidable, (mapIndex(a), mapIndex(b), mapIndex(c))))
        
        return Mesh(self.material, VertexBuffer(vbuffer[0], len(used)), npoly)
    
    def optimize(self):
        def pushmesh():
            outmeshes.append(self.subset(cpoly))
        outmeshes = []
        cpoly, cverts = [], set()
        for collidable, (a, b, c) in self.polygons:
            na, nb, nc = a in cverts, b in cverts, c in cverts
            if len(cverts) + na + nb + nc > 65536:
                pushmesh()
                cpoly, cverts = [], set()
                na = nb = nc = True
            if na: cverts.add(a)
            if nb: cverts.add(b)
            if nc: cverts.add(c)
            cpoly.append((collidable, (a, b, c)))
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
                mat = mesh.material.filenames, mesh.material.flags
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
                k = k.replace('.dds', '.png')
            zip.writestr(k, v)
        if resample:
            print 'Done'
        
        for obj in self.objects:
            obj.meshes = [x for m in obj.meshes for x in m.optimize()]
        
        def ouint(*x):
            zout.write(struct.pack('<' + 'I'*len(x), *x))
        def ofloat(*x):
            zout.write(struct.pack('<' + 'f'*len(x), *x))
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
                    mat = mesh.material.flags, mesh.material.filenames
                    if mat not in materials:
                        materials[mat] = len(materials)
            ouint(len(materials))
            for (flags, filenames), id in sorted(materials.items(), cmp=lambda a, b: cmp(a[1], b[1])):
                ouint(flags)
                ouint(len(filenames))
                for filename in filenames:
                    ostring(filename)
            ouint(len(self.objects))
            objrefs = {}
            for i, obj in enumerate(self.objects):
                objrefs[obj] = i
                ouint(len(obj.meshes))
                for mesh in obj.meshes:
                    matid = materials[(mesh.material.flags, mesh.material.filenames)]
                    ouint(matid)
                    ouint(len(mesh.vertbuffer))
                    ofloat(*mesh.vertbuffer.data)
                    ouint(len(mesh.polygons))
                    for collidable, x in mesh.polygons:
                        ouint(*x)
                    for collidable, x in mesh.polygons:
                        ouint(int(collidable))

            ouint(len(self.placeables))
            for placeable in self.placeables:
                ouint(objrefs[placeable.obj])
                ofloat(*placeable.position)
                ofloat(*placeable.rotation)
                ofloat(*placeable.scale)
            
            zout.seek(0)
            zip.writestr('zone.oez', zout.read())
