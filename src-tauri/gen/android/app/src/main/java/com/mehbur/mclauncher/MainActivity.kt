package com.mehbur.mclauncher

import android.graphics.Color
import android.os.Bundle
import android.view.View
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Koyu arayüz: durum ve gezinme çubuğu simgeleri açık renk.
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.dark(Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)

    // Arayüz durum çubuğunun, gezinme çubuğunun ve klavyenin altında kalmasın:
    // içerik bu alanlar kadar içeriden başlar (K103). Arka plan --mc-bg rengi,
    // böylece çubukların arkası da uygulamayla aynı görünür.
    val content = findViewById<View>(android.R.id.content)
    content.setBackgroundColor(Color.rgb(5, 7, 10))
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      val ime = insets.getInsets(WindowInsetsCompat.Type.ime())
      view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
      WindowInsetsCompat.CONSUMED
    }
  }
}
