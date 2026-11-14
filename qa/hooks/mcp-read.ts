import { startMockCollab } from "./mock-collab.ts";

export default function startMcpReadMock(): Promise<void> {
	return startMockCollab("mcp-read");
}
