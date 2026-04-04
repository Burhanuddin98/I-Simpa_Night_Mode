#pragma once

#include "mesh/scene_model.h"

namespace isimpa {

void InitViewport();
void DrawViewport(float width, float height);
void ShutdownViewport();

// Load a model into the viewport (also syncs Project.model)
void ViewportLoadModel(const SceneModel& model);

// Re-upload the Project's model to GPU (after color/material changes)
void ViewportRefreshGPU();

// Focus camera on the loaded model
void ViewportFocusModel();

// Get the active scene model (for outliner, properties, etc.)
SceneModel& GetActiveModel();
GPUMesh& GetActiveGPUMesh();

// Viewport toggles
void ViewportToggleWireframe();
void ViewportToggleFaces();
bool ViewportGetShowWireframe();
bool ViewportGetShowFaces();
void ViewportSetFaceMode(int mode); // 0=both, 1=outside, 2=inside
int  ViewportGetFaceMode();

// Camera presets
void ViewportCameraTop();
void ViewportCameraFront();
void ViewportCameraRight();
void ViewportCameraReset();

// Face picking: returns group index at screen coords, or -1
int ViewportPickFace(float screenX, float screenY);

// Surface receiver colormap overlay
struct SurfRecFace;
struct SurfaceRecResult;
void ViewportLoadColormap(const SurfaceRecResult& result);
void ViewportClearColormap();
bool ViewportHasColormap();

// Screenshot
bool ViewportSaveScreenshot(const std::string& path);

// Get world position from screen click (ray-plane intersection at Y=height)
glm::vec3 ViewportScreenToWorld(float screenX, float screenY, float groundY = 0);

// Clipping plane
void ViewportSetClipPlane(bool enabled, int axis, float value);

// Particle animation
struct ParticleData;
void ViewportLoadParticles(const ParticleData& data);
void ViewportClearParticles();
void ViewportSetParticleTimeStep(int step);
int  ViewportGetParticleMaxSteps();
bool ViewportHasParticles();

// Measurement mode (set by App, read by viewport)
extern bool g_viewportMeasureMode;
extern int  g_viewportMeasureState; // 1=waiting A, 2=waiting B
extern glm::vec3 g_viewportMeasureA, g_viewportMeasureB;

} // namespace isimpa
