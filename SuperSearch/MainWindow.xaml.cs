using SuperSearch.Interop;
using SuperSearch.Utilities;
using SuperSearch.ViewModels;
using System;
using System.ComponentModel;
using System.Drawing;
using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Interop;
using Forms = System.Windows.Forms;

namespace SuperSearch;

public partial class MainWindow : Window
{
    private readonly MainViewModel _viewModel;
    private HwndSource? _hwndSource;
    private const int HOTKEY_ID = 0xA113;
    private readonly (HotKeyModifiers Modifiers, Key Key, string Display)[] _hotKeyCandidates =
    {
        (HotKeyModifiers.MOD_ALT, Key.Space, "Alt+Space"),
        (HotKeyModifiers.MOD_CONTROL, Key.Space, "Ctrl+Space"),
        (HotKeyModifiers.MOD_CONTROL | HotKeyModifiers.MOD_SHIFT, Key.Space, "Ctrl+Shift+Space")
    };

    private string _currentHotKeyDisplay = string.Empty;
    private bool _hotKeyRegistered;
    private bool _isShuttingDown;
    private Forms.NotifyIcon? _notifyIcon;
    private Forms.ContextMenuStrip? _notifyMenu;

    public MainWindow(MainViewModel viewModel)
    {
        Log.Info("MainWindow ctor");
        InitializeComponent();
        Log.Info("InitializeComponent complete");

        _viewModel = viewModel;
        DataContext = _viewModel;

        InitializeNotifyIcon();
        Log.Info("Notify icon created");
    }

    public void PrepareForShutdown()
    {
        Log.Info("PrepareForShutdown");
        _isShuttingDown = true;

        if (_hwndSource is not null && _hotKeyRegistered)
        {
            HotKeyInterop.UnregisterHotKey(_hwndSource.Handle, HOTKEY_ID);
            _hotKeyRegistered = false;
        }

        if (_notifyIcon is not null)
        {
            _notifyIcon.Visible = false;
            _notifyIcon.Dispose();
            _notifyIcon = null;
        }

        if (_notifyMenu is not null)
        {
            _notifyMenu.Dispose();
            _notifyMenu = null;
        }
    }

    public void ForceShow()
    {
        Log.Info("ForceShow invoked");
        ShowSearchWindow();
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        Log.Info("OnSourceInitialized");

        _hwndSource = PresentationSource.FromVisual(this) as HwndSource;
        if (_hwndSource is null)
        {
            Log.Error("HWND source is null");
            return;
        }

        _hwndSource.AddHook(WndProc);
        MicaWindowHelper.TryApplyMica(this);
        RegisterHotKeyWithFallback();
    }

    protected override void OnClosing(CancelEventArgs e)
    {
        Log.Info($"OnClosing shuttingDown={_isShuttingDown}");
        if (!_isShuttingDown)
        {
            e.Cancel = true;
            HideWindow();
            return;
        }

        base.OnClosing(e);
    }

    protected override void OnClosed(EventArgs e)
    {
        Log.Info("OnClosed");

        if (_hwndSource is not null)
        {
            if (_hotKeyRegistered)
            {
                HotKeyInterop.UnregisterHotKey(_hwndSource.Handle, HOTKEY_ID);
                _hotKeyRegistered = false;
            }

            _hwndSource.RemoveHook(WndProc);
            _hwndSource = null;
        }

        base.OnClosed(e);
    }

    private async void SearchBox_KeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Escape)
        {
            Log.Info("Escape pressed - hide window");
            HideWindow();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Down)
        {
            MoveSelection(1);
            ResultsList.Focus();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Up)
        {
            MoveSelection(-1);
            ResultsList.Focus();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Enter)
        {
            bool launched;
            if ((Keyboard.Modifiers & ModifierKeys.Shift) == ModifierKeys.Shift)
            {
                Log.Info("Shift+Enter pressed - force search");
                launched = await _viewModel.ForceSearchAsync();
            }
            else
            {
                Log.Info("Enter pressed - execute selection");
                launched = await _viewModel.TryExecuteSelectionAsync();
            }

            if (launched)
            {
                HideWindow();
            }

            e.Handled = true;
        }
    }

    private async void ListBox_PreviewKeyDown(object sender, System.Windows.Input.KeyEventArgs e)
    {
        if (e.Key == Key.Enter)
        {
            bool launched;
            if ((Keyboard.Modifiers & ModifierKeys.Shift) == ModifierKeys.Shift)
            {
                Log.Info("List Shift+Enter pressed");
                launched = await _viewModel.ForceSearchAsync();
            }
            else
            {
                Log.Info("List enter pressed");
                launched = await _viewModel.TryExecuteSelectionAsync();
            }

            if (launched)
            {
                HideWindow();
            }

            e.Handled = true;
            return;
        }

        if (e.Key == Key.Escape)
        {
            Log.Info("List escape pressed");
            HideWindow();
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Down)
        {
            MoveSelection(1);
            e.Handled = true;
            return;
        }

        if (e.Key == Key.Up)
        {
            if (ResultsList.SelectedIndex <= 0)
            {
                SearchBox.Focus();
                SearchBox.SelectAll();
            }
            else
            {
                MoveSelection(-1);
            }
            e.Handled = true;
        }
    }

    private async void ListBox_MouseDoubleClick(object sender, MouseButtonEventArgs e)
    {
        Log.Info("Result double clicked");
        var launched = await _viewModel.TryExecuteSelectionAsync();
        if (launched)
        {
            HideWindow();
        }
    }

    private void MoveSelection(int delta)
    {
        if (ResultsList.Items.Count == 0)
        {
            return;
        }

        var index = ResultsList.SelectedIndex;
        if (index < 0)
        {
            index = delta > 0 ? 0 : ResultsList.Items.Count - 1;
        }
        else
        {
            index = Math.Max(0, Math.Min(ResultsList.Items.Count - 1, index + delta));
        }

        ResultsList.SelectedIndex = index;
        ResultsList.ScrollIntoView(ResultsList.SelectedItem);
    }

    private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        const int WM_HOTKEY = 0x0312;
        const int WM_SYSKEYDOWN = 0x0104;
        const int WM_SYSKEYUP = 0x0105;
        const int WM_SYSCHAR = 0x0106;
        const int WM_CLOSE = 0x0010;
        const int WM_SYSCOMMAND = 0x0112;
        const int SC_CLOSE = 0xF060;
        const int SC_KEYMENU = 0xF100;
        const int SC_HOTKEY = 0xF150;

        bool altSpaceActive = _hotKeyRegistered &&
                              _currentHotKeyDisplay.Contains("Alt+Space", StringComparison.OrdinalIgnoreCase);

        if (altSpaceActive)
        {
            if (msg == WM_SYSCOMMAND)
            {
                int command = wParam.ToInt32() & 0xFFF0;
                if (command is SC_KEYMENU or SC_CLOSE or SC_HOTKEY)
                {
                    handled = true;
                    Log.Info($"Suppressed system command 0x{command:X}");
                    return IntPtr.Zero;
                }
            }

            if (msg == WM_CLOSE && !_isShuttingDown)
            {
                handled = true;
                Log.Info("Suppressed WM_CLOSE while Alt+Space active");
                return IntPtr.Zero;
            }

            if (msg is WM_SYSKEYDOWN or WM_SYSKEYUP or WM_SYSCHAR)
            {
                if (wParam.ToInt32() == KeyInterop.VirtualKeyFromKey(Key.Space))
                {
                    handled = true;
                    Log.Info("Suppressed system key event for Space");
                    return IntPtr.Zero;
                }
            }
        }
        else if (msg == WM_CLOSE && !_isShuttingDown)
        {
            handled = true;
            Log.Info("Suppressed WM_CLOSE");
            return IntPtr.Zero;
        }

        if (msg == WM_HOTKEY && wParam.ToInt32() == HOTKEY_ID)
        {
            Log.Info("Hotkey triggered");
            ToggleWindow();
            handled = true;
            return IntPtr.Zero;
        }

        return IntPtr.Zero;
    }

    private void RegisterHotKeyWithFallback()
    {
        if (_hwndSource is null)
        {
            Log.Error("Cannot register hotkey - hwnd source null");
            return;
        }

        string? notification = null;

        for (int i = 0; i < _hotKeyCandidates.Length; i++)
        {
            var candidate = _hotKeyCandidates[i];
            bool registered = HotKeyInterop.RegisterHotKey(_hwndSource.Handle, HOTKEY_ID, candidate.Modifiers, KeyInterop.VirtualKeyFromKey(candidate.Key));
            Log.Info($"Register hotkey {candidate.Display} result={registered}");
            if (!registered)
            {
                continue;
            }

            _hotKeyRegistered = true;
            _currentHotKeyDisplay = candidate.Display;
            SearchBox.ToolTip = GetHotKeyDisplay();

            if (i > 0)
            {
                notification = $"Fallback hotkey {_currentHotKeyDisplay} activated.";
            }
            else if (!_currentHotKeyDisplay.Equals("Alt+Space", StringComparison.OrdinalIgnoreCase))
            {
                notification = $"{GetHotKeyDisplay()} registered. System menu suppression is enabled.";
            }

            break;
        }

        if (!_hotKeyRegistered)
        {
            notification = "Failed to register any global hotkey. Use the tray icon to open Super Search and configure a custom combo.";
            SearchBox.ToolTip = "Unavailable";
            _currentHotKeyDisplay = "Unavailable";
            Log.Error("Failed to register any global hotkey");
        }

        if (!string.IsNullOrEmpty(notification))
        {
            Dispatcher.BeginInvoke(new Action(() =>
                System.Windows.MessageBox.Show(this, notification, "Super Search", MessageBoxButton.OK, MessageBoxImage.Information)));
        }
    }

    private void ToggleWindow()
    {
        Log.Info($"ToggleWindow visible={IsVisible}");
        if (IsVisible)
        {
            HideWindow();
        }
        else
        {
            ShowSearchWindow();
        }
    }

    private void ShowSearchWindow()
    {
        Log.Info("ShowSearchWindow called");

        if (!IsVisible)
        {
            Show();
        }

        UpdateLayout();
        PositionWindow();

        WindowState = WindowState.Normal;
        Activate();
        Topmost = true;
        Topmost = false;
        Topmost = true;
        SearchBox.Focus();
        SearchBox.SelectAll();
        _viewModel.OnWindowShown();
    }

    private void PositionWindow()
    {
        var workArea = SystemParameters.WorkArea;
        var targetLeft = workArea.Left + (workArea.Width - ActualWidth) / 2;
        var targetTop = workArea.Top + 120;

        Left = Math.Max(workArea.Left, targetLeft);
        Top = Math.Max(workArea.Top + 40, targetTop);
    }

    private void HideWindow()
    {
        if (_isShuttingDown)
        {
            Log.Info("HideWindow ignored during shutdown");
            return;
        }

        Log.Info("HideWindow executing");
        _viewModel.OnWindowHidden();
        Hide();
    }

    private string GetHotKeyDisplay()
    {
        return string.IsNullOrWhiteSpace(_currentHotKeyDisplay) ? "Alt+Space" : _currentHotKeyDisplay;
    }

    private Icon LoadApplicationIcon()
    {
        try
        {
            var iconPath = Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "Assets", "app.ico");
            if (File.Exists(iconPath))
            {
                return new Icon(iconPath, new System.Drawing.Size(32, 32));
            }
        }
        catch (Exception ex)
        {
            Log.Error("Failed to load tray icon", ex);
        }

        return SystemIcons.Application;
    }

    private void InitializeNotifyIcon()
    {
        Log.Info("InitializeNotifyIcon begin");
        _notifyMenu = new Forms.ContextMenuStrip();

        var showItem = new Forms.ToolStripMenuItem("Show Search", null, (_, _) => Dispatcher.Invoke(ShowSearchWindow));
        var hideItem = new Forms.ToolStripMenuItem("Hide", null, (_, _) => Dispatcher.Invoke(HideWindow));
        var exitItem = new Forms.ToolStripMenuItem("Exit", null, (_, _) => Dispatcher.Invoke(() =>
        {
            Log.Info("Exit clicked from tray");
            _isShuttingDown = true;
            System.Windows.Application.Current.Shutdown();
        }));

        _notifyMenu.Items.Add(showItem);
        _notifyMenu.Items.Add(hideItem);
        _notifyMenu.Items.Add(new Forms.ToolStripSeparator());
        _notifyMenu.Items.Add(exitItem);

        _notifyIcon = new Forms.NotifyIcon
        {
            Icon = LoadApplicationIcon(),
            Visible = true,
            Text = "Super Search",
            ContextMenuStrip = _notifyMenu
        };

        _notifyIcon.DoubleClick += (_, _) => Dispatcher.Invoke(ShowSearchWindow);
        _notifyIcon.BalloonTipClicked += (_, _) => Dispatcher.Invoke(ShowSearchWindow);
        _notifyIcon.ShowBalloonTip(3000, "Super Search", $"Use {GetHotKeyDisplay()} (or the tray icon) to open the launcher.", Forms.ToolTipIcon.Info);
    }
}
