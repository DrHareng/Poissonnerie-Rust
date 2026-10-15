package fr.poissonnerie.organisateur;

import android.os.Bundle;
import android.view.View;
import androidx.core.splashscreen.SplashScreen;
import com.getcapacitor.BridgeActivity;

public class MainActivity extends BridgeActivity {
    @Override
    protected void onCreate(Bundle savedInstanceState) {
        SplashScreen splashScreen = SplashScreen.installSplashScreen(this);
        super.onCreate(savedInstanceState);
        splashScreen.setOnExitAnimationListener(provider -> {
            provider.remove();
            clearHandleBackdrop();
        });
        clearHandleBackdrop();
    }

    private void clearHandleBackdrop() {
        getWindow().setBackgroundDrawable(null);
        View decor = getWindow().getDecorView();
        decor.setBackground(null);
        View content = decor.findViewById(android.R.id.content);
        if (content != null) {
            content.setBackground(null);
        }
    }
}
