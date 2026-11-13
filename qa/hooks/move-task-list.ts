import { startMockCollab } from "./mock-collab.ts";

export default function startMoveTaskListMock(): Promise<void> {
	return startMockCollab("move-task-list");
}
