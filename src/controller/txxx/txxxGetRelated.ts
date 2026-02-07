import { Request, Response } from "express";
import LustPress from "../../LustPress";
import { maybeError } from "../../utils/modifier";
import { logger } from "../../utils/logger";
import { ISearchVideoData } from "../../interfaces";

const lust = new LustPress();

function getRelatedApiUrl(videoId: string, page = 1, count = 50): string {
  const id = Number(videoId);
  if (Number.isNaN(id)) throw new Error("Invalid video id");

  const million = Math.floor(id / 1_000_000) * 1_000_000;
  const thousand = Math.floor(id / 1_000) * 1_000;

  return (
    `https://txxx.com/api/json/videos_related2/` +
    `432000/${count}/${million}/${thousand}/${id}.all.${page}.json`
  );
}

export async function relatedTxxx(req: Request, res: Response) {
  try {
    const id = String(req.query.id || "").trim();
    const page = Number(req.query.page || 1);

    if (!id) throw new Error("Parameter id is required");
    if (Number.isNaN(page)) throw new Error("Parameter page must be a number");

    const apiUrl = getRelatedApiUrl(id, page);

    const buffer = await lust.fetchBody(apiUrl);
    const rawData = JSON.parse(buffer.toString("utf-8"));

    const videos = Array.isArray(rawData.videos) ? rawData.videos : [];

    const data = videos.map((v: any) => ({
      id: v.video_id,
      title: v.title,
      image: v.scr || v.thumb || null,
      duration: v.duration || "None",
      views: v.video_viewed || "0",
      rating: v.rating || "0",
      uploader: v.username || v.display_name || "",
      link: `https://txxx.com/videos/${v.video_id}/${v.dir}/`,
      video: `https://txxx.com/embed/${v.video_id}/`,
    }));

    logger.info({
      path: req.path,
      query: req.query,
      method: req.method,
      ip: req.ip,
      useragent: req.get("User-Agent"),
    });

    return res.json({
      success: true,
      total_count: rawData.total_count || "0",
      pages: rawData.pages || 1,
      page,
      data,
      source: `https://txxx.com/videos/${id}/`,
    } as ISearchVideoData);
  } catch (err) {
    const e = err as Error;
    return res.status(400).json(maybeError(false, e.message));
  }
}
