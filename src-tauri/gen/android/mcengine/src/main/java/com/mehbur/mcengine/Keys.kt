package com.mehbur.mcengine

import android.view.KeyEvent

/** Android tuş kodlarından GLFW tuş kodlarına eşleme (oyunun anladığı dil). */
object Keys {
    const val GLFW_KEY_ESCAPE = 256
    const val GLFW_KEY_ENTER = 257
    const val GLFW_KEY_BACKSPACE = 259
    const val GLFW_MOUSE_BUTTON_LEFT = 0
    const val GLFW_MOUSE_BUTTON_RIGHT = 1

    private val map: Map<Int, Int> = buildMap {
        for (i in 0..25) put(KeyEvent.KEYCODE_A + i, 65 + i) // A-Z
        for (i in 0..9) put(KeyEvent.KEYCODE_0 + i, 48 + i) // 0-9
        for (i in 0..11) put(KeyEvent.KEYCODE_F1 + i, 290 + i) // F1-F12
        put(KeyEvent.KEYCODE_SPACE, 32)
        put(KeyEvent.KEYCODE_APOSTROPHE, 39)
        put(KeyEvent.KEYCODE_COMMA, 44)
        put(KeyEvent.KEYCODE_MINUS, 45)
        put(KeyEvent.KEYCODE_PERIOD, 46)
        put(KeyEvent.KEYCODE_SLASH, 47)
        put(KeyEvent.KEYCODE_SEMICOLON, 59)
        put(KeyEvent.KEYCODE_EQUALS, 61)
        put(KeyEvent.KEYCODE_LEFT_BRACKET, 91)
        put(KeyEvent.KEYCODE_BACKSLASH, 92)
        put(KeyEvent.KEYCODE_RIGHT_BRACKET, 93)
        put(KeyEvent.KEYCODE_GRAVE, 96)
        put(KeyEvent.KEYCODE_ESCAPE, GLFW_KEY_ESCAPE)
        put(KeyEvent.KEYCODE_BACK, GLFW_KEY_ESCAPE)
        put(KeyEvent.KEYCODE_ENTER, GLFW_KEY_ENTER)
        put(KeyEvent.KEYCODE_NUMPAD_ENTER, GLFW_KEY_ENTER)
        put(KeyEvent.KEYCODE_TAB, 258)
        put(KeyEvent.KEYCODE_DEL, GLFW_KEY_BACKSPACE)
        put(KeyEvent.KEYCODE_INSERT, 260)
        put(KeyEvent.KEYCODE_FORWARD_DEL, 261)
        put(KeyEvent.KEYCODE_DPAD_RIGHT, 262)
        put(KeyEvent.KEYCODE_DPAD_LEFT, 263)
        put(KeyEvent.KEYCODE_DPAD_DOWN, 264)
        put(KeyEvent.KEYCODE_DPAD_UP, 265)
        put(KeyEvent.KEYCODE_PAGE_UP, 266)
        put(KeyEvent.KEYCODE_PAGE_DOWN, 267)
        put(KeyEvent.KEYCODE_MOVE_HOME, 268)
        put(KeyEvent.KEYCODE_MOVE_END, 269)
        put(KeyEvent.KEYCODE_CAPS_LOCK, 280)
        put(KeyEvent.KEYCODE_SHIFT_LEFT, 340)
        put(KeyEvent.KEYCODE_CTRL_LEFT, 341)
        put(KeyEvent.KEYCODE_ALT_LEFT, 342)
        put(KeyEvent.KEYCODE_SHIFT_RIGHT, 344)
        put(KeyEvent.KEYCODE_CTRL_RIGHT, 345)
        put(KeyEvent.KEYCODE_ALT_RIGHT, 346)
    }

    /** GLFW kodu; bilinmeyen tuşta 0. */
    fun glfw(androidKeyCode: Int): Int = map[androidKeyCode] ?: 0
}
