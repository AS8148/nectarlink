# Screen video implementation

Windows uses Media Foundation for screen encoding and video decoding. It checks
each adapter's decoder profile and enumerates screen encoders for that adapter's
LUID. Screen mirroring and the webcam share decoder selection; webcam textures
retain their producer's adapter LUID when passed to the virtual camera.

When decoding switches devices, the receiver requests a keyframe before
resuming.
