// three.js's TSL, loosely typed. @types/three types every node by its WGSL type and rejects graphs the
// shader compilers accept (an int loop index mixed into float arithmetic, a uniform array's element, a
// bitcast's result in integer logic). The graphs are checked where they compile, on the GPU: a shader
// that does not build fails the e2e's pixel and read-back hooks.
import * as TSL from 'three/tsl';

export const T = TSL as unknown as Record<keyof typeof TSL, any>;
