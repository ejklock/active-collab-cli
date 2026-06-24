"""Tests for models — from_api() parsers against the ActiveCollab 7.2.25 payload shapes."""

import unittest

from active_collab.models import Comment, Instance, MineTask, Project, Task

# Real payload shapes from SKILL.md / legacy test fixtures.

TASK_SINGLE = {
    "id": 75159,
    "task_number": 42,
    "name": "Implement login flow",
    "is_completed": False,
    "is_trashed": False,
    "assignee_id": 486,
    "project_id": 665,
    "body": "<p>The login page needs to be implemented. &amp; done.</p>",
    "start_on": 1780963200,
    "due_on": 1780963200,
    "estimate": 0.0,
}

COMMENT_DATA = {
    "id": 1,
    "body": "<p>Great start on this feature.</p>",
    "body_plain_text": "Great start on this feature.",
    "created_by_name": "Alice",
    "created_by_id": 5,
    "created_by_email": "alice@example.com",
    "created_on": 1736499600,
}

PROJECT_DATA = {
    "id": 665,
    "name": "My Project",
    "is_trashed": False,
}

MINE_TASK_DATA = {
    "id": 75159,
    "task_number": 42,
    "name": "Implement login flow",
    "is_completed": False,
    "is_trashed": False,
    "assignee_id": 7,
    "project_id": 665,
}


class TestTaskFromApi(unittest.TestCase):
    def test_parses_id_and_name(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.id, 75159)
        self.assertEqual(task.name, "Implement login flow")

    def test_parses_task_number(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.task_number, 42)

    def test_parses_is_completed_false(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertFalse(task.is_completed)

    def test_parses_is_completed_true(self) -> None:
        task = Task.from_api({**TASK_SINGLE, "is_completed": True})
        self.assertTrue(task.is_completed)

    def test_parses_assignee_id(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.assignee_id, 486)

    def test_parses_project_id(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.project_id, 665)

    def test_parses_body(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertIn("login page", task.body)

    def test_parses_timestamps(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.start_on, 1780963200)
        self.assertEqual(task.due_on, 1780963200)

    def test_parses_estimate(self) -> None:
        task = Task.from_api(TASK_SINGLE)
        self.assertEqual(task.estimate, 0.0)

    def test_tracked_time_from_top_level_payload(self) -> None:
        task = Task.from_api(TASK_SINGLE, tracked_time=3.0)
        self.assertEqual(task.tracked_time, 3.0)

    def test_missing_assignee_id_defaults_to_none(self) -> None:
        single = {k: v for k, v in TASK_SINGLE.items() if k != "assignee_id"}
        task = Task.from_api(single)
        self.assertIsNone(task.assignee_id)

    def test_missing_start_on_defaults_to_none(self) -> None:
        single = {k: v for k, v in TASK_SINGLE.items() if k != "start_on"}
        task = Task.from_api(single)
        self.assertIsNone(task.start_on)

    def test_missing_due_on_defaults_to_none(self) -> None:
        single = {k: v for k, v in TASK_SINGLE.items() if k != "due_on"}
        task = Task.from_api(single)
        self.assertIsNone(task.due_on)

    def test_missing_estimate_defaults_to_none(self) -> None:
        single = {k: v for k, v in TASK_SINGLE.items() if k != "estimate"}
        task = Task.from_api(single)
        self.assertIsNone(task.estimate)

    def test_missing_body_defaults_to_empty_string(self) -> None:
        single = {k: v for k, v in TASK_SINGLE.items() if k != "body"}
        task = Task.from_api(single)
        self.assertEqual(task.body, "")

    def test_empty_dict_does_not_raise(self) -> None:
        task = Task.from_api({})
        self.assertEqual(task.id, 0)
        self.assertEqual(task.name, "")

    def test_extra_keys_are_ignored(self) -> None:
        data = {**TASK_SINGLE, "unknown_future_field": "value"}
        task = Task.from_api(data)
        self.assertEqual(task.id, 75159)


class TestCommentFromApi(unittest.TestCase):
    def test_parses_id_and_body(self) -> None:
        comment = Comment.from_api(COMMENT_DATA)
        self.assertEqual(comment.id, 1)
        self.assertEqual(comment.body, "<p>Great start on this feature.</p>")

    def test_parses_body_plain_text(self) -> None:
        comment = Comment.from_api(COMMENT_DATA)
        self.assertEqual(comment.body_plain_text, "Great start on this feature.")

    def test_parses_created_by_name(self) -> None:
        comment = Comment.from_api(COMMENT_DATA)
        self.assertEqual(comment.created_by_name, "Alice")

    def test_parses_created_by_id(self) -> None:
        comment = Comment.from_api(COMMENT_DATA)
        self.assertEqual(comment.created_by_id, 5)

    def test_parses_created_on(self) -> None:
        comment = Comment.from_api(COMMENT_DATA)
        self.assertEqual(comment.created_on, 1736499600)

    def test_missing_body_plain_text_defaults_to_empty(self) -> None:
        data = {k: v for k, v in COMMENT_DATA.items() if k != "body_plain_text"}
        comment = Comment.from_api(data)
        self.assertEqual(comment.body_plain_text, "")

    def test_missing_created_by_name_defaults_to_empty(self) -> None:
        data = {k: v for k, v in COMMENT_DATA.items() if k != "created_by_name"}
        comment = Comment.from_api(data)
        self.assertEqual(comment.created_by_name, "")

    def test_missing_created_on_defaults_to_none(self) -> None:
        data = {k: v for k, v in COMMENT_DATA.items() if k != "created_on"}
        comment = Comment.from_api(data)
        self.assertIsNone(comment.created_on)

    def test_empty_dict_does_not_raise(self) -> None:
        comment = Comment.from_api({})
        self.assertEqual(comment.id, 0)


class TestProjectFromApi(unittest.TestCase):
    def test_parses_id_and_name(self) -> None:
        project = Project.from_api(PROJECT_DATA)
        self.assertEqual(project.id, 665)
        self.assertEqual(project.name, "My Project")

    def test_parses_is_trashed_false(self) -> None:
        project = Project.from_api(PROJECT_DATA)
        self.assertFalse(project.is_trashed)

    def test_parses_is_trashed_true(self) -> None:
        project = Project.from_api({**PROJECT_DATA, "is_trashed": True})
        self.assertTrue(project.is_trashed)

    def test_missing_name_defaults_to_empty(self) -> None:
        project = Project.from_api({"id": 1})
        self.assertEqual(project.name, "")

    def test_empty_dict_does_not_raise(self) -> None:
        project = Project.from_api({})
        self.assertEqual(project.id, 0)


class TestMineTaskFromApi(unittest.TestCase):
    def test_parses_id_name_and_project_id(self) -> None:
        task = MineTask.from_api(MINE_TASK_DATA)
        self.assertEqual(task.id, 75159)
        self.assertEqual(task.name, "Implement login flow")
        self.assertEqual(task.project_id, 665)

    def test_parses_task_number(self) -> None:
        task = MineTask.from_api(MINE_TASK_DATA)
        self.assertEqual(task.task_number, 42)

    def test_parses_is_completed_false(self) -> None:
        task = MineTask.from_api(MINE_TASK_DATA)
        self.assertFalse(task.is_completed)

    def test_parses_is_trashed_false(self) -> None:
        task = MineTask.from_api(MINE_TASK_DATA)
        self.assertFalse(task.is_trashed)

    def test_completed_task_sets_flag(self) -> None:
        task = MineTask.from_api({**MINE_TASK_DATA, "is_completed": True})
        self.assertTrue(task.is_completed)

    def test_trashed_task_sets_flag(self) -> None:
        task = MineTask.from_api({**MINE_TASK_DATA, "is_trashed": True})
        self.assertTrue(task.is_trashed)

    def test_instance_name_attached(self) -> None:
        task = MineTask.from_api(MINE_TASK_DATA, instance_name="prod")
        self.assertEqual(task.instance_name, "prod")

    def test_missing_task_number_defaults_to_none(self) -> None:
        data = {k: v for k, v in MINE_TASK_DATA.items() if k != "task_number"}
        task = MineTask.from_api(data)
        self.assertIsNone(task.task_number)

    def test_empty_dict_does_not_raise(self) -> None:
        task = MineTask.from_api({})
        self.assertEqual(task.id, 0)


class TestInstanceFromRow(unittest.TestCase):
    def test_parses_all_fields(self) -> None:
        row = ("myinst", "https://collab.example.com", "user@example.com", "TOKEN123", 7)
        inst = Instance.from_row(row)
        self.assertEqual(inst.name, "myinst")
        self.assertEqual(inst.base_url, "https://collab.example.com")
        self.assertEqual(inst.email, "user@example.com")
        self.assertEqual(inst.token, "TOKEN123")
        self.assertEqual(inst.user_id, 7)

    def test_user_id_none_is_allowed(self) -> None:
        row = ("myinst", "https://collab.example.com", "user@example.com", "TOKEN123", None)
        inst = Instance.from_row(row)
        self.assertIsNone(inst.user_id)


if __name__ == "__main__":
    unittest.main(verbosity=2)
