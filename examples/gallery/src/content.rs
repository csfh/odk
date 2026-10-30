//! What the gallery shows: friendly demo data, and the worst realistic case, swapped at this
//! one boundary so every component meets the same data it would in an application. The worst
//! case is plausible on purpose: long German compounds, an unbroken URL, Japanese and emoji,
//! one-letter labels, long translated button text, deep paths and many tabs.

/// Every string the gallery renders, by where it appears.
pub struct Content {
    pub chords: [&'static str; 2],
    /// Secondary, primary and ghost badges, then info, success, warning and danger.
    pub badges: [&'static str; 7],
    /// An unchecked, a checked, a mixed and a disabled checkbox.
    pub checkboxes: [&'static str; 4],
    /// An on, an off and a disabled switch.
    pub switches: [&'static str; 3],
    /// The text inputs: the placeholder of an empty one, the values of a filled, a read-only
    /// and a disabled one, and what precedes the echo of the first.
    pub inputs: [&'static str; 5],
    /// The text areas: the placeholder of an empty one, and the values of a filled and a
    /// read-only one, which have several lines.
    pub textareas: [&'static str; 3],
    /// The input groups: the text before and after the field, its placeholder and the label of
    /// the button.
    pub groups: [&'static str; 4],
    /// The command palette: the placeholder, five commands (the third is disabled) and what
    /// shows when nothing matches.
    pub commands: (&'static str, [&'static str; 5], &'static str),
    /// The selects: the placeholder, four options (the third is disabled) and what precedes
    /// the echo of the first.
    pub select: (&'static str, [&'static str; 4], &'static str),
    /// The tooltips of a Save button and a close button.
    pub tooltips: [&'static str; 2],
    /// A dialog's title, description and its two actions.
    pub dialog: (&'static str, &'static str, [&'static str; 2]),
    pub buttons: [&'static str; 3],
    /// A label too long for the narrow button that fills a small width.
    pub long_button: &'static str,
    /// Title, description and action label for the info, success, warning and danger alerts.
    pub alerts: [(&'static str, &'static str, &'static str); 4],
    pub short_path: &'static [&'static str],
    pub long_path: &'static [&'static str],
    /// A resizable group's three panels: each one's title and body.
    pub panels: [(&'static str, &'static str); 3],
    /// The names a data table's processes take, in turn.
    pub processes: &'static [&'static str],
    /// Three items: each one's title, description and the line under them (none when empty).
    pub items: [(&'static str, &'static str, &'static str); 3],
    /// The group the items are in: its title and description.
    pub item_group: (&'static str, &'static str),
    /// An empty state: its title, its description and its action's label.
    pub empty: (&'static str, &'static str, &'static str),
    /// A group in a box that lists two things: its title, its description, and each thing's name
    /// and what it says of it.
    pub outline_group: (
        &'static str,
        &'static str,
        [(&'static str, &'static str); 2],
    ),
    pub alignment: [&'static str; 4],
    pub tabs: &'static [&'static str],
    /// Three periods for a segmented control: day, week and month.
    pub periods: [&'static str; 3],
    /// A sidebar: two group headings and five rows, the first three in the first group.
    pub nav: ([&'static str; 2], [&'static str; 5]),
    /// The labels of an active, an inactive, a successful, a warning, a failed and an
    /// informational status.
    pub statuses: [&'static str; 6],
    /// A progress bar's name.
    pub progress: &'static str,
    /// Label and key hint for the File menu's items.
    pub file_menu: [(&'static str, &'static str); 5],
    pub context_target: &'static str,
    pub hover: (&'static str, &'static str, &'static str),
    pub sheet: (&'static str, &'static str, [&'static str; 2]),
    /// Title, description and action label for toasts.
    pub toast: (&'static str, &'static str, &'static str),
    /// The example form.
    pub form: Form,
}

/// The example form's text: each field's label, description and messages, the options of its
/// radio group and its actions.
pub struct Form {
    pub name: (&'static str, &'static str),
    /// The email field's label, its placeholder and the error when it has no `@`.
    pub email: (&'static str, &'static str, &'static str),
    /// The plan's label and its three options (the third is disabled).
    pub plan: (&'static str, [&'static str; 3]),
    /// The seats field's label and its help.
    pub seats: (&'static str, &'static str),
    /// The notes field's label and its warning.
    pub notes: (&'static str, &'static str),
    /// The agreement's label and the error while it is not checked.
    pub agree: (&'static str, &'static str),
    /// The success message once the form is sent, and the two actions.
    pub done: &'static str,
    pub actions: [&'static str; 2],
}

impl Content {
    /// The demo content, or the worst case.
    pub fn get(worst_case: bool) -> Self {
        if worst_case {
            Self::worst_case()
        } else {
            Self::demo()
        }
    }

    pub fn demo() -> Self {
        Self {
            chords: ["Ctrl+K", "Ctrl+Shift++"],
            badges: [
                "Synced", "New", "Draft", "Updating", "Online", "Low disk", "Failed",
            ],
            switches: ["Wi-Fi", "Bluetooth", "Airplane mode"],
            inputs: [
                "Your name",
                "notes.md",
                "/home/aleksandra/Documents",
                "aleksandra",
                "You typed:",
            ],
            groups: ["https://", ".com", "example", "Copy"],
            commands: (
                "Type a command…",
                [
                    "Open file",
                    "Save file",
                    "Save as…",
                    "Close window",
                    "Rename",
                ],
                "No results",
            ),
            textareas: [
                "What happened?",
                "Shopping:\n- oat milk\n- coffee\n- a very long list that needs to wrap onto a second row\n- stamps\n- batteries",
                "Read me first.\n\nThis one only scrolls and copies.",
            ],
            select: (
                "Choose a country",
                ["Denmark", "Sweden", "Norway", "Finland"],
                "You chose:",
            ),
            tooltips: ["Save the file (Ctrl+S)", "Close"],
            dialog: (
                "Rename file",
                "Choose a new name for notes.md.",
                ["Cancel", "Rename"],
            ),
            checkboxes: [
                "Show hidden files",
                "Wrap long lines",
                "Select all",
                "Managed by your organisation",
            ],
            buttons: ["Save", "Delete", "Reset"],
            long_button: "Save changes and continue",
            alerts: [
                ("Backup scheduled", "Runs tonight at 02:00.", ""),
                ("Saved", "", ""),
                ("Disk almost full", "12 GB left on /home.", "Clean up"),
                ("Backend unavailable", "ratmos did not answer.", ""),
            ],
            short_path: &["Settings", "Display", "Scaling"],
            long_path: &["Home", "Settings", "Hardware", "Input", "Keyboard layout"],
            panels: [
                ("Services", "NetworkManager, bluetooth, sshd and nine more."),
                (
                    "bluetooth.service",
                    "Active since 09:12. Bluetooth service for the adapter hci0.",
                ),
                ("Log", "09:12 Starting Bluetooth service…"),
            ],
            processes: &[
                "hyprland", "waybar", "firefox", "atmosd", "pipewire", "kitty",
            ],
            items: [
                (
                    "Natural scroll",
                    "Content moves with your fingers, the way a phone does.",
                    "~/.config/hypr/input.lua · input.touchpad.natural_scroll",
                ),
                (
                    "Acceleration",
                    "Adaptive speeds up as you move. Flat keeps a steady ratio.",
                    "~/.config/hypr/input.lua · input.accel_profile",
                ),
                (
                    "Sensitivity",
                    "Pointer speed. Zero is the Hyprland default. Negative is slower.",
                    "~/.config/hypr/input.lua · input.sensitivity",
                ),
            ],
            item_group: (
                "Pointer",
                "Sensitivity and acceleration for the mouse and trackpad.",
            ),
            empty: (
                "No matching processes",
                "Nothing matches that search. Clear it to see every process again.",
                "Clear search",
            ),
            outline_group: (
                "Paired devices",
                "Devices this computer has connected to before.",
                [
                    ("WH-1000XM5", "Headphones. Connected."),
                    ("MX Master 3S", "Mouse. Last seen yesterday."),
                ],
            ),
            alignment: ["Left", "Center", "Right", "Justify"],
            tabs: &["General", "Display", "Advanced", "About"],
            periods: ["Day", "Week", "Month"],
            nav: (
                ["Workspace", "Account"],
                ["General", "Members", "Billing", "Profile", "Notifications"],
            ),
            statuses: [
                "Online",
                "Offline",
                "Synced",
                "Degraded",
                "Build failed",
                "Updating",
            ],
            progress: "Copying photos",
            file_menu: [
                ("New", "Ctrl+N"),
                ("Open…", "Ctrl+O"),
                ("Word wrap", ""),
                ("Print", ""),
                ("Delete", ""),
            ],
            context_target: "Right-click here, or focus and press Menu",
            hover: (
                "@omarchy",
                "Omarchy",
                "An opinionated Arch and Hyprland setup.",
            ),
            sheet: (
                "Display settings",
                "Tab stays inside this sheet; Escape closes it.",
                ["Apply", "Cancel"],
            ),
            toast: ("Saved", "Dismisses itself after a few seconds.", "Retry"),
            form: Form {
                name: ("Full name", "As it appears on your ID"),
                email: ("Email", "name@example.com", "Enter an address with an @"),
                plan: ("Plan", ["Personal", "Team", "Enterprise"]),
                seats: ("Seats", "Between 1 and 25"),
                notes: (
                    "Notes",
                    "Notes are visible to everyone in your organisation",
                ),
                agree: ("I accept the terms", "Accept the terms to continue"),
                done: "Account created",
                actions: ["Cancel", "Create account"],
            },
        }
    }

    pub fn worst_case() -> Self {
        Self {
            chords: ["Ctrl+Alt+Shift+Super+PageDown", "Strg+Umschalt+Ä"],
            badges: [
                "1.284",
                "N",
                "Entwurf",
                "Wird aktualisiert…",
                "オンライン",
                "Speicherplatz knapp",
                "Fehlgeschlagen 🔥",
            ],
            dialog: (
                "Datei umbenennen und in einen anderen Ordner verschieben",
                "Wählen Sie einen neuen Namen für Besprechungsnotizen-2026-10-20-endgültig-überarbeitet.md.",
                ["Abbrechen", "Umbenennen und verschieben"],
            ),
            tooltips: [
                "Speichert alle geöffneten Dokumente und synchronisiert sie mit https://cloud.example.com/aleksandra.wisniewska-kowalczyk",
                "閉じる",
            ],
            switches: [
                "Automatisch mit bekannten Netzwerken verbinden, auch im Energiesparmodus",
                "B",
                "機内モード（管理者によって無効化）",
            ],
            inputs: [
                "Anzeigename für externe Bildschirme und Projektoren",
                "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed",
                "/home/aleksandra/Dokumente/Steuererklärung/日本語のとても長いフォルダー名/😀",
                "Verwaltet von Ihrer Organisation",
                "Eingegeben:",
            ],
            groups: [
                "https://wiki.omarchy.org/troubleshooting/backups/",
                "日本語のとても長いドメイン名.example.com 😀",
                "Pneumonoultramicroscopicsilicovolcanoconiosis",
                "In die Zwischenablage kopieren",
            ],
            commands: (
                "Beschreiben Sie, was Sie tun möchten, zum Beispiel die Sicherung wiederherstellen",
                [
                    "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed",
                    "日本語のとても長いコマンドの名前がここに入ります 😀😀😀",
                    "Datei unter einem anderen Namen speichern (nicht verfügbar)",
                    "Fenster schließen",
                    "Pneumonoultramicroscopicsilicovolcanoconiosis",
                ],
                "Keine Ergebnisse für diese Suche in den Befehlen der Anwendung",
            ),
            textareas: [
                "Beschreiben Sie, was beim Wiederherstellen der Sicherung auf den externen Bildschirmen und Projektoren passiert ist",
                "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed\n日本語のとても長い文章がここに続きます。折り返されるはずです。😀😀😀😀😀😀😀😀😀😀\n\nPneumonoultramicroscopicsilicovolcanoconiosis\nEnde",
                "Nur lesbar: /home/aleksandra/Dokumente/Steuererklärung/日本語のとても長いフォルダー名/😀\nZweite Zeile",
            ],
            select: (
                "Land oder Region für die Steuererklärung auswählen",
                [
                    "Bundesrepublik Deutschland (Deutschland)",
                    "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed",
                    "日本国（日本語のとても長いラベルは折り返さずに省略されます）",
                    "🇩🇰 Königreich Dänemark 🇩🇰",
                ],
                "Ausgewählt:",
            ),
            checkboxes: [
                "隠しファイルを表示",
                "Automatischer Zeilenumbruch am Fensterrand, auch in eingebetteten Ansichten",
                "A",
                "Von Ihrer Organisation verwaltet 🔒",
            ],
            buttons: [
                "Änderungen speichern und fortfahren",
                "X",
                "Auf Werkseinstellungen zurücksetzen",
            ],
            long_button: "Änderungen speichern und zum nächsten Schritt fortfahren",
            alerts: [
                (
                    "Datensicherungsüberprüfungsprotokoll wurde erstellt",
                    "See https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check-failed-after-interrupted-snapshot",
                    "",
                ),
                ("✓", "", ""),
                (
                    "ディスクの空き容量が少なくなっています",
                    "/home/aleksandra.wisniewska-kowalczyk には 12 GB しか残っていません。",
                    "Speicherplatz jetzt freigeben",
                ),
                ("Backend unavailable 🔥🔥🔥", "", ""),
            ],
            short_path: &["S", "Ä", "日本語の設定ページ"],
            long_path: &[
                "Home",
                "Systemeinstellungen",
                "Hardware und Peripheriegeräte",
                "Eingabegeräte",
                "Tastaturbelegungen und Eingabemethoden",
                "Deutsch (Schweiz, ohne Akzenttasten)",
                "Erweiterte Tastenkombinationen für Barrierefreiheit",
            ],
            panels: [
                (
                    "Systemdienstverwaltungsübersicht",
                    "https://example.com/a/very/long/unbroken/path/to/a/unit/file/that/will/not/wrap/anywhere.service",
                ),
                (
                    "日本語のサービス詳細 🔥",
                    "サービスは起動していません。ログを確認してください。🔥🔥",
                ),
                ("L", "x"),
            ],
            items: [
                (
                    "Natürliches Bildlaufverhalten für Berührungsflächen und Zeigegeräte",
                    "Inhalte folgen den Fingern wie auf einem Telefon. Diese Beschreibung ist absichtlich so lang, dass sie in einer schmalen Spalte mehrmals umbricht und neben ihr nichts verdrängt.",
                    "~/.config/hypr/a/very/long/unbroken/path/to/an/input/configuration/file/that/will/not/wrap/anywhere.lua",
                ),
                (
                    "日本語の設定 🔥",
                    "ポインターの加速。🔥🔥",
                    "~/.config/hypr/入力.lua",
                ),
                ("L", "x", ""),
            ],
            outline_group: (
                "Gekoppelte Geräte, Kopfhörer, Mäuse, Tastaturen und Controller 🎧",
                "Geräte, mit denen dieser Rechner schon verbunden war. Auch diese Beschreibung bricht in einer schmalen Spalte mehrmals um.",
                [
                    (
                        "ワイヤレスノイズキャンセリングヘッドホン",
                        "ヘッドホン。接続済み。",
                    ),
                    ("x", ""),
                ],
            ),
            empty: (
                "Keine passenden Prozesse, Dienste, Ports oder Fenster in dieser Ansicht 🔥",
                "Nichts passt zu dieser Suche. Diese Beschreibung ist absichtlich so lang, dass sie auch auf einer breiten Seite mehrmals umbricht, statt quer über die ganze Breite zu laufen.",
                "Suche und alle Filter zurücksetzen",
            ),
            item_group: (
                "Zeigegeräte, Berührungsflächen und alles, was den Mauszeiger bewegt 🔥",
                "Empfindlichkeit und Beschleunigung. Diese Beschreibung ist absichtlich so lang, dass sie in einer schmalen Spalte mehrmals umbricht, bevor das erste Element beginnt.",
            ),
            processes: &[
                "x",
                "Hintergrunddienstverwaltungsprozessüberwachung",
                "/usr/lib/a/very/long/unbroken/path/to/an/executable/that/never/wraps",
                "日本語のプロセス名",
                "🔥🔥🔥",
            ],
            alignment: [
                "Linksbündig",
                "Zentriert",
                "R",
                "Blocksatz mit Silbentrennung",
            ],
            tabs: &[
                "Allgemein",
                "Anzeige und Bildschirmskalierung",
                "Erweitert",
                "Über",
                "Netzwerk",
                "Datenschutz und Sicherheit",
                "Barrierefreiheit",
                "Energie",
                "日本語",
                "🔊",
            ],
            periods: ["Tag", "Kalenderwoche", "日本語の月"],
            nav: (
                [
                    "Arbeitsbereich und gemeinsame Einstellungen",
                    "日本語のアカウント設定",
                ],
                [
                    "Allgemein",
                    "Mitglieder und Berechtigungen der Organisation",
                    "Abrechnung",
                    "P",
                    "Benachrichtigungen 🔔",
                ],
            ),
            statuses: [
                "Online seit 12 Tagen",
                "オフライン",
                "Synchronisiert",
                "Eingeschränkt verfügbar wegen Wartungsarbeiten",
                "Build fehlgeschlagen 🔥",
                "I",
            ],
            progress: "Fotos werden auf https://cloud.example.com/aleksandra kopiert",
            file_menu: [
                ("Neues Dokument aus Vorlage erstellen", "Ctrl+Alt+Shift+N"),
                ("Öffnen…", "Strg+O"),
                ("Automatischer Zeilenumbruch am Fensterrand", ""),
                ("Drucken", ""),
                ("Dauerhaft löschen, ohne Papierkorb", "Shift+Delete"),
            ],
            context_target: "Rechtsklick hier, oder fokussieren und die Menütaste drücken",
            hover: (
                "@aleksandra.wisniewska-kowalczyk",
                "Aleksandra Wiśniewska-Kowalczyk",
                "bartholomew.fitzgerald@northwind-industries-holdings.example.com",
            ),
            sheet: (
                "Anzeigeeinstellungen für externe Bildschirme und Projektoren",
                "Die Tabulatortaste bleibt in diesem Fenster; Escape schließt es.",
                ["Übernehmen und schließen", "Abbrechen"],
            ),
            toast: (
                "Die Datensicherung wurde erfolgreich abgeschlossen 🎉",
                "https://wiki.omarchy.org/troubleshooting/backups/restic-repository-integrity-check",
                "Erneut versuchen",
            ),
            form: Form {
                name: (
                    "Vollständiger Name des Kontoinhabers oder der Kontoinhaberin",
                    "So, wie er auf Ihrem amtlichen Lichtbildausweis steht, einschließlich aller Bindestriche und Namenszusätze",
                ),
                email: (
                    "E-Mail-Adresse",
                    "aleksandra.wisniewska-kowalczyk@northwind-industries-holdings.example.com",
                    "Geben Sie eine vollständige E-Mail-Adresse mit einem @ ein, zum Beispiel name@example.com",
                ),
                plan: (
                    "Tarif",
                    [
                        "Persönlich",
                        "Team (bis zu fünfundzwanzig Arbeitsplätze, jährliche Abrechnung)",
                        "企業向け（営業担当者にお問い合わせください）",
                    ],
                ),
                seats: (
                    "Arbeitsplätze",
                    "Zwischen 1 und 25; Pfeiltasten erhöhen und verringern",
                ),
                notes: (
                    "Anmerkungen für die Buchhaltung",
                    "Anmerkungen sind für alle Mitglieder Ihrer Organisation sichtbar 😀",
                ),
                agree: (
                    "Ich habe die Nutzungsbedingungen und die Datenschutzerklärung gelesen und akzeptiere sie",
                    "Sie müssen den Bedingungen zustimmen, um fortzufahren",
                ),
                done: "Ihr Konto wurde angelegt; eine Bestätigung ist unterwegs",
                actions: ["Abbrechen", "Konto anlegen und fortfahren"],
            },
        }
    }
}
