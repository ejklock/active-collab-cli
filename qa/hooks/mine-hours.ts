import { startMockCollab } from "./mock-collab.ts";

export default function startMineHoursMock(): Promise<void> {
	return startMockCollab("mine-hours");
}
