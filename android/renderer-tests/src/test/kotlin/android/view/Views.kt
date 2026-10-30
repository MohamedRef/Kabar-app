package android.view

interface ViewParent

open class View {
    var parent: ViewParent? = null
        internal set
}

open class ViewGroup : View(), ViewParent {
    private val children = mutableListOf<View>()
    val childCount: Int get() = children.size
    fun getChildAt(index: Int): View = children[index]
    fun addView(child: View) {
        require(child.parent == null)
        children.add(child)
        child.parent = this
    }
    fun removeView(child: View) {
        if (children.remove(child)) child.parent = null
    }
}

class FrameLayout : ViewGroup()
class Window(val decorView: View)
