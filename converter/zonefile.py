import hashlib, struct, tempfile

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
    def __init__(self, flags, data):
        self.flags = flags
        self.filename = hashlib.sha256(data).hexdigest() + '.dds' 
        self.data = data

class Mesh(object):
    def __init__(self, material, vertbuffer, polygons):
        self.material = material
        self.vertbuffer = vertbuffer
        self.polygons = polygons
    
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
    
    def output(self, zip):
        assets = {}
        for obj in self.objects:
            for mesh in obj.meshes:
                material = mesh.material
                if material.filename not in assets:
                    assets[material.filename] = material.data
        for k, v in assets.items():
            zip.writestr(k, v)
        
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
                    mat = mesh.material.flags, mesh.material.filename
                    if mat not in materials:
                        materials[mat] = len(materials)
            ouint(len(materials))
            for (flags, filename), id in sorted(materials.items(), cmp=lambda a, b: cmp(a[1], b[1])):
                ouint(flags)
                ostring(filename)
            ouint(len(self.objects))
            objrefs = {}
            for i, obj in enumerate(self.objects):
                objrefs[obj] = i
                ouint(len(obj.meshes))
                for mesh in obj.meshes:
                    matid = materials[(mesh.material.flags, mesh.material.filename)]
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
