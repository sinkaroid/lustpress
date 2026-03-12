import { Request, Response } from "express";
import LustPress from "../../LustPress";
import { maybeError } from "../../utils/modifier";
import { logger } from "../../utils/logger";
import { IVideoData } from "../../interfaces";

const lust = new LustPress();

// Generate sharded API URL exactly like TXXX expects
function getApiUrl(videoId: string): string {
  const id = Number(videoId);
  if (Number.isNaN(id)) throw new Error("Invalid video id");

  const million = Math.floor(id / 1_000_000) * 1_000_000;
  const thousand = Math.floor(id / 1_000) * 1_000;

  return `https://txxx.com/api/json/video/86400/${million}/${thousand}/${id}.json`;
}

export async function getTxxx(req: Request, res: Response) {
  try {
    const id = String(req.query.id || "").trim();
    if (!id) throw new Error("Parameter id is required");

    const apiUrl = getApiUrl(id);

    const buffer = await lust.fetchBody(apiUrl);
    const parsed = JSON.parse(buffer.toString("utf-8"));

    if (!parsed?.video) {
      throw new Error("Invalid API response");
    }

    const video = parsed.video;

    const categories = Object.values(video.categories || {}).map(
      (c: any) => c.title,
    );

    const tags = Object.values(video.tags || {}).map((t: any) => t.title);

    const models = Object.values(video.models || {}).map((m: any) => m.title);

    const videoDir = video.dir || "";
    const videoId = video.video_id;

    const embed = `https://txxx.com/embed/${videoId}/`;

    const response: IVideoData = {
      success: true,
      data: {
        title: video.title || "None",
        id: `${videoId}`,
        image: video.thumbsrc || video.thumb || "None",
        duration: video.duration || "None",
        views: video.statistics?.viewed || "0",
        rating: video.statistics?.rating || "0.00",
        uploaded: video.post_date || "None",
        upvoted: String(video.statistics?.likes || "0"),
        downvoted: String(video.statistics?.dislikes || "0"),
        channel: video.channel?.title || "",
        models: models.length > 0 ? models : video.models_suggested || [],
        tags: [...categories, ...tags],
      },
      source: `https://txxx.com/videos/${videoId}/${videoDir}/`,
      assets: [embed, video.thumbsrc].filter(Boolean),
    };

    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent"),
    });

    return res.json(response);
  } catch (err) {
    const e = err as Error;
    return res.status(400).json(maybeError(false, e.message));
  }
}
