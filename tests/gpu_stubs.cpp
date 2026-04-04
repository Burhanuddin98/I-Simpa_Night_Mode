// GPU mesh stubs for headless testing
#include "mesh/scene_model.h"

namespace isimpa {

void GPUMesh::Upload(const SceneModel&, const std::set<int>&) {}
void GPUMesh::Draw(GLuint, const glm::mat4&, bool) {}
void GPUMesh::Destroy() {}
GLuint CreateMeshShader() { return 0; }
GLuint CreateWireShader() { return 0; }

void ViewportLoadModel(const SceneModel&) {}
void ViewportRefreshGPU() {}

} // namespace isimpa
