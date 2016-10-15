var gl;

var vshader, fshader, program;

function setup() {
    gl.enable(gl.DEPTH_TEST);
    gl.clearColor(0, 0, 0, 255);

    $.ajax('vertshader.glsl?_' + Date.now()).done(function(data) {
        vshader = data;
        compileShader();
    });
    $.ajax('fragshader.glsl?_' + Date.now()).done(function(data) {
        fshader = data;
        compileShader();
    });
}

// Shamelessly ripped from http://webglfundamentals.org/webgl/lessons/webgl-boilerplate.html
function compileOneShader(shaderSource, shaderType) {
  // Create the shader object
  var shader = gl.createShader(shaderType);
 
  // Set the shader source code.
  gl.shaderSource(shader, shaderSource);
 
  // Compile the shader
  gl.compileShader(shader);
 
  // Check if it compiled
  var success = gl.getShaderParameter(shader, gl.COMPILE_STATUS);
  if (!success) {
    // Something went wrong during compilation; get the error
    throw "could not compile shader:" + gl.getShaderInfoLog(shader);
  }
 
  return shader;
}
function compileShader() {
    if(vshader === undefined || fshader === undefined || program !== undefined)
        return;
    
    program = gl.createProgram();

    // attach the shaders.
    gl.attachShader(program, compileOneShader(vshader, gl.VERTEX_SHADER));
    gl.attachShader(program, compileOneShader(fshader, gl.FRAGMENT_SHADER));

    // link the program.
    gl.linkProgram(program);

    // Check if it linked.
    var success = gl.getProgramParameter(program, gl.LINK_STATUS);
    if (!success) {
        // something went wrong with the link
        throw ("program filed to link:" + gl.getProgramInfoLog (program));
    }

    return program;
}

var pMatrix = mat4.create(), mvMatrix = mat4.create();

var n = 0;
function render() {
    if(!loading && program !== undefined && frameBuffers !== undefined && frameBuffers[0] !== undefined) {
        gl.viewport(0, 0, gl.viewportWidth, gl.viewportHeight);
        gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

        mat4.perspective(pMatrix, 45, gl.viewportWidth / gl.viewportHeight, 1, 50.0);
        mat4.identity(mvMatrix);
        mat4.translate(mvMatrix, mvMatrix, [0, -3, -15]);
        mat4.rotate(mvMatrix, mvMatrix, Math.PI / 2, [0, 1, 0]);
        mat4.rotate(mvMatrix, mvMatrix, Math.PI / 2, [-1, 0, 0]);
        mat4.rotate(mvMatrix, mvMatrix, Math.PI + n++ / 50, [0, 0, -1]);

        gl.useProgram(program);
        gl.uniformMatrix4fv(gl.getUniformLocation(program, 'PMatrix'), false, pMatrix);
        gl.uniformMatrix4fv(gl.getUniformLocation(program, 'MVMatrix'), false, mvMatrix);

        var tdelta = (Date.now() - startTime) / 1000 / (slow ? 5 : 1);
        var framenum = ~~(tdelta * 10);
        var framepos = interpolate ? (tdelta - (framenum / 10)) * 10 : 0;
        
        gl.uniform1f(gl.getUniformLocation(program, 'framepos'), framepos);

        gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
        gl.bindBuffer(gl.ARRAY_BUFFER, frameBuffers[framenum % frameBuffers.length]);

        gl.vertexAttribPointer(a = gl.getAttribLocation(program, 'vPosition'), 3, gl.FLOAT, false, 6*4, 0*4);
        gl.vertexAttribPointer(b = gl.getAttribLocation(program, 'vNormal'), 3, gl.FLOAT, false, 6*4, 3*4);
        gl.enableVertexAttribArray(a);
        gl.enableVertexAttribArray(b);
        if(interpolate && ((framenum + 1) % frameBuffers.length) != 0) {
            gl.bindBuffer(gl.ARRAY_BUFFER, frameBuffers[(framenum + 1) % frameBuffers.length]);
        }
        gl.vertexAttribPointer(a = gl.getAttribLocation(program, 'vNextPosition'), 3, gl.FLOAT, false, 6*4, 0*4);
        gl.vertexAttribPointer(b = gl.getAttribLocation(program, 'vNextNormal'), 3, gl.FLOAT, false, 6*4, 3*4);
        gl.enableVertexAttribArray(a);
        gl.enableVertexAttribArray(b);

        gl.drawElements(gl.TRIANGLES, polycount, gl.UNSIGNED_SHORT, 0);
    }

    window.requestAnimationFrame(render);
}

var models = {}, curModel, loading = true;
function loadModel(name) {
    if(!models[name]) {
        loading = true;
        $('#msg').text('Loading model...');
        $.ajax(name + '.json').done(function(data) {
            models[name] = data;
            $('#msg').text('');
            setupModel(data);
        });
    } else {
        setupModel(models[name]);
    }
}

var indexBuffer, polycount;
function setupModel(data) {
    loading = false;
    curModel = data;
    $('#animations').empty();

    var anis = [];
    for(var k in data)
        anis.push(k);
    anis = anis.sort();
    for(var k of anis) {
        if(k != 'polys')
            $('#animations').append('<option>' + k + '</option>');
    }
    
    indexBuffer = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, indexBuffer);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint16Array(data['polys']), gl.STATIC_DRAW);
    polycount = data['polys'].length;
    
    setupAnimation('ani_');
}

var frameBuffers, curFrame;
var startTime;

function setupAnimation(name) {
    startTime = Date.now();

    var ani = curModel[name];
    frameBuffers = [];
    curFrame = 0;
    for(var frame in ani['frames']) {
        var verts = ani['frames'][frame];
        var vb = frameBuffers[frame] = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, vb);
        gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(verts), gl.STATIC_DRAW);
    }
}

var interpolate = false, slow = false;

$(document).ready(function() {
    var cvs = $('#cvs')[0];
    gl = cvs.getContext('webgl');
    gl.viewportWidth = cvs.width;
    gl.viewportHeight = cvs.height;

    $('#model').change(function() {
        loadModel($('#model').val());
    });
    $('#animations').change(function() {
        setupAnimation($('#animations').val());
    });
    $('#interpolate').change(function() {
        interpolate = this.checked;
    });
    $('#slow').change(function() {
        slow = this.checked;
    });
    loadModel('orc');

    setup();
    render();
});