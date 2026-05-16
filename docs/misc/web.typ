#title[Paiagram Web]

#import "../links.typ": links

= Using the Web Version

You can directly #link(links.home)[open the web version] in your browser. The experience is mostly the same on both web
and native. There may be a \~10% yet negligible performance penalty for the web version.

As a result, you get instant update and immediate access to the nightly version if using the web version.

= Windows Specific Issues

== The app is slow on my device

Chrome uses the default power profile, which tends to save power aggressively. You could make chrome use the
"High-performance" power profile by enabling
#link("chrome://flags/#force-high-performance-gpu")[`chrome://flags/#force-high-performance-gpu`]

== My browser does not support WebGPU

If you are on Windows 7, Windows 8, or Windows 8.1 then sorry -- your device does not support WebGPU. Consider upgrading
to Windows 10 or 11, or #link("https://fedoraproject.org/")[use Linux instead].

If you are on Windows 10 or Windows 11, make sure that you are using the latest version of Chrome. Afterwards, navigate
to `chrome://flags/` then set "Unsafe WebGPU" to true.

= Linux Specific Issues

== My browser does not support WebGPU

Please refer to your specific browser's troubleshooting guide. The author of Paiagram has tested these browsers on some
devices:

- Firefox: supports WebGPU on an Intel iGPU. Doesn't support Nvidia.
  - Works with wayland on the iGPU.
- Google Chrome: supports WebGPU.
  - Works on an Intel iGPU.
  - Works on an Nvidia 4060 mobile. This requires enabling Vulkan, Vulkan ANGLE layer, and X.org. Wayland would not
    work.
