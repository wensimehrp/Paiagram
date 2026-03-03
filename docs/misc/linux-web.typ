#import "../book.typ": book-page, links

#show: book-page.with(title: "Paiagram Web")

= Using the Web Version

You can directly #link(links.home)[open the web version] in your browser. The
experience is the same on both web and native.

= Web Troubleshooting

The web version of Paiagram requires #link("https://developer.chrome.com/docs/web-platform/webgpu")[WebGPU], which isn't
enabled by default on Linux Chromium and other browsers.

As a result, Windows 7, Windows 8, and Windows 8.1 are not supported, since Chromium and Firefox don't provide WebGPU
support for those platforms. If you couldn't run Windows 10 or 11 on your hardware,
#link("https://fedoraproject.org/")[consider switching to Linux].

== Enabling WebGPU (Linux)

You can enable WebGPU using the following command line arguments:

```sh
chromium --enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan
```

For Google Chrome, replace `chromium` with `google-chrome`.

If you are using an NVIDIA GPU and WebGPU still does not work, try forcing X11:

```sh
chromium --ozone-platform=x11 --enable-unsafe-webgpu --enable-features=Vulkan --use-angle=vulkan
```

After launching the browser with these flags, open `chrome://gpu` and confirm that WebGPU is listed as available.
