package app.sahifa.reader

import android.app.Application
import app.sahifa.reader.core.Core

class SahifaApp : Application() {
    override fun onCreate() {
        super.onCreate()
        Core.init(this)
    }
}
