package com.mehbur.mclauncher

import android.content.Intent
import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.widget.Toast
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
    showGameExit(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    showGameExit(intent)
  }

  /** Oyun (K104) hatayla kapandıysa kısa bir uyarı; günlük Profil → Günlükler'de. */
  private fun showGameExit(intent: Intent?) {
    val code = intent?.getIntExtra(EXIT_CODE, 0) ?: 0
    intent?.removeExtra(EXIT_CODE)
    if (code != 0) {
      Toast.makeText(this, getString(R.string.game_exit_error, code), Toast.LENGTH_LONG).show()
    }
  }

  private companion object {
    // net.kdt.pojavlaunch.ExitActivity.EXTRA_EXIT_CODE ile aynı.
    const val EXIT_CODE = "com.mehbur.mcengine.EXIT_CODE"
  }
}
