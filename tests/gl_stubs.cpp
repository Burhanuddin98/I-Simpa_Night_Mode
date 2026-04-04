// OpenGL function stubs for headless testing (no GPU required)
// These provide link-time symbols for code that references GL types/functions

// Stub GLuint type and GL functions used in scene_model.h (GPUMesh struct)
// The actual GPU mesh code won't be called in tests since we stub ViewportLoadModel

// Minimal GL type definitions to satisfy the linker
typedef unsigned int GLenum;
typedef unsigned int GLuint;
typedef int GLint;
typedef int GLsizei;
typedef float GLfloat;
typedef unsigned char GLboolean;

// Stub all GL functions referenced by any linked code
extern "C" {
    GLuint glCreateShader(GLenum) { return 0; }
    void glShaderSource(GLuint, GLsizei, const char**, const GLint*) {}
    void glCompileShader(GLuint) {}
    void glGetShaderiv(GLuint, GLenum, GLint* p) { if(p) *p = 1; }
    void glGetShaderInfoLog(GLuint, GLsizei, GLsizei*, char*) {}
    GLuint glCreateProgram() { return 0; }
    void glAttachShader(GLuint, GLuint) {}
    void glLinkProgram(GLuint) {}
    void glGetProgramiv(GLuint, GLenum, GLint* p) { if(p) *p = 1; }
    void glGetProgramInfoLog(GLuint, GLsizei, GLsizei*, char*) {}
    void glDeleteShader(GLuint) {}
    void glDeleteProgram(GLuint) {}
    void glGenVertexArrays(GLsizei, GLuint*) {}
    void glGenBuffers(GLsizei, GLuint*) {}
    void glBindVertexArray(GLuint) {}
    void glBindBuffer(GLenum, GLuint) {}
    void glBufferData(GLenum, long long, const void*, GLenum) {}
    void glEnableVertexAttribArray(GLuint) {}
    void glVertexAttribPointer(GLuint, GLint, GLenum, GLboolean, GLsizei, const void*) {}
    void glDeleteVertexArrays(GLsizei, const GLuint*) {}
    void glDeleteBuffers(GLsizei, const GLuint*) {}
    void glUseProgram(GLuint) {}
    void glUniformMatrix4fv(GLint, GLsizei, GLboolean, const GLfloat*) {}
    void glUniform3f(GLint, GLfloat, GLfloat, GLfloat) {}
    void glUniform1f(GLint, GLfloat) {}
    GLint glGetUniformLocation(GLuint, const char*) { return -1; }
    void glDrawArrays(GLenum, GLint, GLsizei) {}
}
