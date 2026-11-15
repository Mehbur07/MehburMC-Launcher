package com.mehbur.mcengine

import android.content.Context
import android.text.InputType
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import org.lwjgl.glfw.CallbackBridge

/**
 * Ekran klavyesinden gelen yazıyı oyuna iletir. Görünmez bir görünümdür;
 * klavye uygulaması ona yazar, o da karakterleri ve silme/enter tuşlarını
 * oyuna gönderir. Öneri/otomatik düzeltme kapalıdır (görünür parola türü),
 * yine de "taslak" (composing) yazı gelirse oyundaki metin onunla eşitlenir.
 */
class KeyboardSink(context: Context) : View(context) {
    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = InputType.TYPE_CLASS_TEXT or
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD or
            InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_DONE
        return Connection()
    }

    private inner class Connection : BaseInputConnection(this, false) {
        /** Oyuna gönderilmiş taslak yazının uzunluğu. */
        private var composing = 0

        private fun erase(n: Int) = repeat(n) { CallbackBridge.sendKeyPress(Keys.GLFW_KEY_BACKSPACE) }

        private fun type(text: CharSequence) {
            for (c in text) {
                if (c == '\n') CallbackBridge.sendKeyPress(Keys.GLFW_KEY_ENTER)
                else CallbackBridge.sendChar(c, CallbackBridge.getCurrentMods())
            }
        }

        override fun commitText(text: CharSequence, newCursorPosition: Int): Boolean {
            erase(composing)
            composing = 0
            type(text)
            return true
        }

        override fun setComposingText(text: CharSequence, newCursorPosition: Int): Boolean {
            erase(composing)
            type(text)
            composing = text.length
            return true
        }

        override fun finishComposingText(): Boolean {
            composing = 0
            return true
        }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            erase(maxOf(beforeLength, 1))
            return true
        }

        override fun performEditorAction(actionCode: Int): Boolean {
            CallbackBridge.sendKeyPress(Keys.GLFW_KEY_ENTER)
            return true
        }
    }
}
