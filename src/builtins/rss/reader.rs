use super::feed::{article_html, Article};

#[cfg(target_os = "macos")]
pub(super) struct Reader {
    window: objc2::rc::Retained<objc2_app_kit::NSWindow>,
    webview: objc2::rc::Retained<objc2_web_kit::WKWebView>,
}

#[cfg(target_os = "macos")]
impl Reader {
    pub fn new() -> Result<Self, String> {
        use objc2_app_kit::{
            NSAutoresizingMaskOptions, NSBackingStoreType, NSStatusWindowLevel, NSView, NSWindow,
            NSWindowStyleMask,
        };
        use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
        use objc2_web_kit::{WKWebView, WKWebViewConfiguration};

        let mtm = MainThreadMarker::new().ok_or("阅读窗口需要在主线程打开")?;
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(900.0, 700.0));
        // AppKit owns event delivery through eframe's existing native event loop.
        // Keep the window and WebView retained until the extension stops.
        let (window, webview) = unsafe {
            let window = NSWindow::initWithContentRect_styleMask_backing_defer(
                mtm.alloc(),
                bounds,
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable
                    | NSWindowStyleMask::Resizable,
                NSBackingStoreType::NSBackingStoreBuffered,
                false,
            );
            window.setReleasedWhenClosed(false);
            // Keep the reader above the capsule's transparent status-level window.
            // Otherwise WebKit can regard its drawing surface as occluded.
            window.setLevel(NSStatusWindowLevel + 1);
            let container = NSView::initWithFrame(mtm.alloc(), bounds);
            container.setWantsLayer(true);
            window.setContentView(Some(&container));
            let configuration = WKWebViewConfiguration::new();
            let webview = WKWebView::initWithFrame_configuration(
                mtm.alloc(),
                container.bounds(),
                &configuration,
            );
            webview.setAutoresizingMask(
                NSAutoresizingMaskOptions::NSViewWidthSizable
                    | NSAutoresizingMaskOptions::NSViewHeightSizable,
            );
            webview.setWantsLayer(true);
            container.addSubview(&webview);
            window.center();
            (window, webview)
        };
        Ok(Self { window, webview })
    }

    pub fn is_closed(&self) -> bool {
        !self.window.isVisible()
    }

    pub fn show(&self, article: &Article, source: &str) -> Result<(), String> {
        use objc2_foundation::{NSString, NSURLRequest, NSURL};

        self.window.setTitle(&NSString::from_str(&article.title));
        unsafe {
            let base = article
                .link
                .as_ref()
                .and_then(|url| NSURL::URLWithString(&NSString::from_str(url)));
            if article.body.trim().is_empty() && base.is_none() {
                return Err("这篇文章没有正文或原文链接".into());
            }
            self.window.makeKeyAndOrderFront(None);
            self.webview.layoutSubtreeIfNeeded();
            if article.body.trim().is_empty() {
                let url = base.as_ref().ok_or("这篇文章没有正文或原文链接")?;
                self.webview.loadRequest(&NSURLRequest::requestWithURL(url));
            } else {
                self.webview.loadHTMLString_baseURL(
                    &NSString::from_str(&article_html(article, source)),
                    base.as_deref(),
                );
            }
            self.webview.setNeedsDisplay(true);
            self.window.displayIfNeeded();
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
impl Drop for Reader {
    fn drop(&mut self) {
        unsafe {
            self.webview.stopLoading();
        }
        self.window.close();
    }
}

#[cfg(not(target_os = "macos"))]
pub(super) struct Reader {
    directory: tempfile::TempDir,
}

#[cfg(not(target_os = "macos"))]
impl Reader {
    pub fn is_closed(&self) -> bool {
        false
    }

    pub fn new() -> Result<Self, String> {
        Ok(Self {
            directory: tempfile::tempdir().map_err(|error| error.to_string())?,
        })
    }

    pub fn show(&self, article: &Article, source: &str) -> Result<(), String> {
        if article.body.trim().is_empty() {
            return webbrowser::open(
                article
                    .link
                    .as_deref()
                    .ok_or("这篇文章没有正文或原文链接")?,
            )
            .map_err(|error| error.to_string());
        }
        let path = self.directory.path().join("article.html");
        std::fs::write(&path, article_html(article, source)).map_err(|error| error.to_string())?;
        let url = url::Url::from_file_path(path).map_err(|_| "无法创建阅读链接")?;
        webbrowser::open(url.as_str()).map_err(|error| error.to_string())
    }
}
