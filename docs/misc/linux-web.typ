#import "../book.typ": book-page

#show: book-page.with(title: "Linux Web Troubleshooting")

The web version of Paiagram requires #link("https://developer.chrome.com/docs/web-platform/webgpu")[WebGPU], which isn't
enabled by default on Linux Chromium and other browsers.

= Enabling WebGPU

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
