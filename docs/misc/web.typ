#title[Paiagram Web]

#import "../links.typ": links

= Using the Web Version

You can directly #link(links.home)[open the web version] in your browser. The experience is the same on both web and
native. The app runs best on Google Chrome and Chromium based-browsers, although other browsers (e.g. Safari) are also
supported.

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

- In general, Non-chromium browsers tend to support WebGPU poorly.
- Try switching to Chromium.
- Use x.org to launch Chromium. I ran into problems trying to let it use Wayland.
- Navigate to `chrome://flags/` then modify the following settings:
  - Unsafe WebGPU: enable
  - Vulkan: enable
