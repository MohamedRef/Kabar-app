package app.kabar

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class BackKeysTest {
    private val asked = mutableListOf<(String) -> Unit>()
    private val replayed = mutableListOf<Pair<String, String>>()
    private val keys = BackKeys<String>(
        ask = { asked.add(it) },
        replay = { down, up -> replayed.add(down to up) },
    )

    private fun press(n: Int) {
        assertTrue(keys.onBack("down$n", isDown = true, repeat = 0, canceled = false))
        assertTrue(keys.onBack("up$n", isDown = false, repeat = 0, canceled = false))
    }

    @Test
    fun a_step_keeps_the_press_inside_the_app() {
        press(1)
        assertEquals(1, asked.size)
        asked[0]("\"step\"")
        assertEquals(emptyList(), replayed)
    }

    @Test
    fun exit_on_today_hands_the_press_to_android() {
        press(1)
        asked[0]("\"exit\"")
        assertEquals(listOf("down1" to "up1"), replayed)
    }

    @Test
    fun no_page_script_keeps_android_back() {
        press(1)
        asked[0]("null")
        assertEquals(listOf("down1" to "up1"), replayed)
    }

    @Test
    fun a_held_key_asks_once_and_a_cancelled_gesture_never() {
        assertTrue(keys.onBack("d", isDown = true, repeat = 0, canceled = false))
        assertTrue(keys.onBack("d2", isDown = true, repeat = 1, canceled = false))
        assertTrue(keys.onBack("u", isDown = false, repeat = 0, canceled = false))
        assertEquals(1, asked.size)
        assertTrue(keys.onBack("d", isDown = true, repeat = 0, canceled = false))
        assertTrue(keys.onBack("u", isDown = false, repeat = 0, canceled = true))
        assertEquals(1, asked.size)
    }

    @Test
    fun an_up_without_its_down_is_not_taken() {
        assertFalse(keys.onBack("u", isDown = false, repeat = 0, canceled = false))
        assertEquals(0, asked.size)
    }
}
