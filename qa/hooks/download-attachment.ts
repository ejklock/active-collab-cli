import { startMockCollab } from "./mock-collab.ts";

export default function startDownloadAttachmentMock(): Promise<void> {
	return startMockCollab("download-attachment");
}
