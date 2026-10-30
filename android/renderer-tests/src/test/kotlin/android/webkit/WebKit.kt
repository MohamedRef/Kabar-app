package android.webkit

import android.graphics.Bitmap
import android.view.View

open class WebView : View() {
    var webViewClient: WebViewClient = WebViewClient()
    var destroyed: Boolean = false
        private set
    fun destroy() { check(!destroyed); destroyed = true }
}

interface WebResourceRequest
class WebResourceError
class WebResourceResponse

open class RenderProcessGoneDetail(private val crashed: Boolean) {
    open fun didCrash(): Boolean = crashed
}

open class WebViewClient {
    open fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? = null
    open fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = false
    open fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) = Unit
    open fun onPageFinished(view: WebView, url: String) = Unit
    open fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) = Unit
    open fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean = false
}
