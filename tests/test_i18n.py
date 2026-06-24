"""Tests for i18n.py — locale resolution and translation lookup."""

import unittest

from active_collab import i18n
from active_collab.i18n import __, _resolve_lang, set_language


class TestTranslationLookup(unittest.TestCase):
    def tearDown(self) -> None:
        set_language("en")

    def test_returns_pt_br_translation_when_lang_is_pt_br(self) -> None:
        set_language("pt_BR")
        self.assertEqual(__("Task"), "Tarefa")

    def test_returns_source_string_for_en(self) -> None:
        set_language("en")
        self.assertEqual(__("Task"), "Task")

    def test_returns_source_string_for_unknown_locale(self) -> None:
        set_language("fr")
        self.assertEqual(__("Task"), "Task")

    def test_returns_source_string_when_key_missing_in_pt_br(self) -> None:
        set_language("pt_BR")
        self.assertEqual(__("Some unlisted key"), "Some unlisted key")

    def test_all_expected_pt_br_entries_present(self) -> None:
        set_language("pt_BR")
        expected_pairs = [
            ("Task", "Tarefa"),
            ("Name", "Nome"),
            ("Status", "Status"),
            ("Assignee", "Responsável"),
            ("Start", "Início"),
            ("Due", "Prazo"),
            ("Estimate", "Estimativa"),
            ("Logged", "Registrado"),
            ("Description", "Descrição"),
            ("(no description)", "(sem descrição)"),
            ("(unassigned)", "(não atribuído)"),
            ("(unknown)", "(desconhecido)"),
            ("Comments", "Comentários"),
            ("Completed", "Concluído"),
            ("Open", "Aberto"),
        ]
        for source, expected in expected_pairs:
            with self.subTest(source=source):
                self.assertEqual(__(source), expected)


class TestResolveLang(unittest.TestCase):
    def test_active_collab_lang_takes_precedence_over_lang(self) -> None:
        env = {"ACTIVE_COLLAB_LANG": "pt_BR", "LANG": "en_US.UTF-8"}
        self.assertEqual(_resolve_lang(env), "pt_BR")

    def test_lang_used_when_active_collab_lang_absent(self) -> None:
        env = {"LANG": "pt_BR.UTF-8"}
        self.assertEqual(_resolve_lang(env), "pt_BR")

    def test_lc_all_used_when_lang_absent(self) -> None:
        env = {"LC_ALL": "pt_BR.UTF-8"}
        self.assertEqual(_resolve_lang(env), "pt_BR")

    def test_active_collab_lang_beats_lc_all(self) -> None:
        env = {"ACTIVE_COLLAB_LANG": "pt_BR", "LC_ALL": "en_US.UTF-8"}
        self.assertEqual(_resolve_lang(env), "pt_BR")

    def test_lang_beats_lc_all(self) -> None:
        env = {"LANG": "pt_BR.UTF-8", "LC_ALL": "en_US.UTF-8"}
        self.assertEqual(_resolve_lang(env), "pt_BR")

    def test_unknown_locale_returns_en(self) -> None:
        env = {"LANG": "fr_FR.UTF-8"}
        self.assertEqual(_resolve_lang(env), "en")

    def test_empty_env_returns_en(self) -> None:
        self.assertEqual(_resolve_lang({}), "en")

    def test_empty_lang_value_returns_en(self) -> None:
        self.assertEqual(_resolve_lang({"LANG": ""}), "en")

    def test_pt_br_utf8_maps_to_pt_br(self) -> None:
        self.assertEqual(_resolve_lang({"ACTIVE_COLLAB_LANG": "pt_BR.UTF-8"}), "pt_BR")

    def test_pt_br_without_suffix_maps_to_pt_br(self) -> None:
        self.assertEqual(_resolve_lang({"ACTIVE_COLLAB_LANG": "pt_BR"}), "pt_BR")

    def test_lc_all_pt_br_utf8_maps_to_pt_br(self) -> None:
        self.assertEqual(_resolve_lang({"LC_ALL": "pt_BR.UTF-8"}), "pt_BR")


class TestSetLanguageRoundTrip(unittest.TestCase):
    def tearDown(self) -> None:
        set_language("en")

    def test_set_and_get_pt_br(self) -> None:
        set_language("pt_BR")
        self.assertEqual(i18n._lang, "pt_BR")

    def test_set_and_get_en(self) -> None:
        set_language("en")
        self.assertEqual(i18n._lang, "en")

    def test_language_affects_translation(self) -> None:
        set_language("pt_BR")
        translated = __("Open")
        set_language("en")
        identity = __("Open")
        self.assertEqual(translated, "Aberto")
        self.assertEqual(identity, "Open")
