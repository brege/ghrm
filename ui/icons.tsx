import type { ReactElement } from 'react';
import { BiData, BiFileFind, BiMoon } from 'react-icons/bi';
import { BsGit, BsGithub, BsThreeDotsVertical } from 'react-icons/bs';
import {
  FaBitbucket,
  FaCode,
  FaFolder,
  FaGitlab,
  FaRegCommentDots,
  FaRegNoteSticky,
} from 'react-icons/fa6';
import {
  FiBookOpen,
  FiCheck,
  FiChevronDown,
  FiChevronLeft,
  FiChevronRight,
  FiChevronUp,
  FiDownload,
  FiPlus,
  FiSearch,
  FiSun,
} from 'react-icons/fi';
import {
  GoAlert,
  GoCopy,
  GoFilter,
  GoHome,
  GoLightBulb,
  GoLocation,
  GoPeople,
  GoScreenFull,
  GoStop,
  GoSync,
  GoTag,
  GoTrash,
  GoVersions,
  GoZoomIn,
  GoZoomOut,
} from 'react-icons/go';
import { IoMdCode } from 'react-icons/io';
import { IoCloudOfflineOutline } from 'react-icons/io5';
import {
  LuCopySlash,
  LuFile,
  LuList,
  LuRabbit,
  LuTable2,
  LuTurtle,
} from 'react-icons/lu';
import {
  MdBorderOuter,
  MdCommit,
  MdLogout,
  MdOutlineBorderClear,
  MdRepeat,
  MdUpdate,
  MdWrapText,
} from 'react-icons/md';
import { PiGitBranch } from 'react-icons/pi';
import { RxExternalLink } from 'react-icons/rx';
import { SiCodeberg, SiSourcehut } from 'react-icons/si';
import { TbEdit, TbEggCracked, TbListSearch, TbScale } from 'react-icons/tb';

type Icon = {
  id: string;
  icon: ReactElement;
};

export const icons: Icon[] = [
  { id: 'ghrm-icon-sun', icon: <FiSun /> },
  { id: 'ghrm-icon-moon', icon: <BiMoon /> },
  { id: 'ghrm-icon-search', icon: <FiSearch /> },
  { id: 'ghrm-icon-dir', icon: <FaFolder /> },
  { id: 'ghrm-icon-file', icon: <LuFile /> },
  { id: 'ghrm-icon-book', icon: <FiBookOpen /> },
  { id: 'ghrm-icon-list', icon: <LuList /> },
  { id: 'ghrm-icon-search-path', icon: <TbListSearch /> },
  { id: 'ghrm-icon-search-content', icon: <BiFileFind /> },
  { id: 'ghrm-icon-doc-framed', icon: <MdBorderOuter /> },
  { id: 'ghrm-icon-doc-flat', icon: <MdOutlineBorderClear /> },
  { id: 'ghrm-icon-github', icon: <BsGithub /> },
  { id: 'ghrm-icon-branch', icon: <PiGitBranch /> },
  { id: 'ghrm-icon-tag', icon: <GoTag /> },
  { id: 'ghrm-icon-version', icon: <GoVersions /> },
  { id: 'ghrm-icon-bitbucket', icon: <FaBitbucket /> },
  { id: 'ghrm-icon-gitlab', icon: <FaGitlab /> },
  { id: 'ghrm-icon-codeberg', icon: <SiCodeberg /> },
  { id: 'ghrm-icon-sourcehut', icon: <SiSourcehut /> },
  { id: 'ghrm-icon-git', icon: <BsGit /> },
  { id: 'ghrm-icon-people', icon: <GoPeople /> },
  { id: 'ghrm-icon-scale', icon: <TbScale /> },
  { id: 'ghrm-icon-location', icon: <GoLocation /> },
  { id: 'ghrm-icon-created', icon: <TbEggCracked /> },
  { id: 'ghrm-icon-table', icon: <LuTable2 /> },
  { id: 'ghrm-icon-update', icon: <MdUpdate /> },
  { id: 'ghrm-icon-commit', icon: <MdCommit /> },
  { id: 'ghrm-icon-repeat', icon: <MdRepeat /> },
  { id: 'ghrm-icon-loc', icon: <IoMdCode /> },
  { id: 'ghrm-icon-data', icon: <BiData /> },
  { id: 'ghrm-icon-code', icon: <FaCode /> },
  { id: 'ghrm-icon-download', icon: <FiDownload /> },
  { id: 'ghrm-icon-zst', icon: <LuRabbit /> },
  { id: 'ghrm-icon-zip', icon: <LuTurtle /> },
  { id: 'ghrm-icon-wrap', icon: <MdWrapText /> },
  { id: 'ghrm-icon-copy', icon: <GoCopy /> },
  { id: 'ghrm-icon-copy-path', icon: <LuCopySlash /> },
  { id: 'ghrm-icon-check', icon: <FiCheck /> },
  { id: 'ghrm-icon-fullscreen', icon: <GoScreenFull /> },
  { id: 'ghrm-icon-zoom-in', icon: <GoZoomIn /> },
  { id: 'ghrm-icon-zoom-out', icon: <GoZoomOut /> },
  { id: 'ghrm-icon-reset', icon: <GoSync /> },
  { id: 'ghrm-icon-filter', icon: <GoFilter /> },
  { id: 'ghrm-icon-columns', icon: <BsThreeDotsVertical /> },
  { id: 'ghrm-icon-chevron-up', icon: <FiChevronUp /> },
  { id: 'ghrm-icon-chevron-down', icon: <FiChevronDown /> },
  { id: 'ghrm-icon-chevron-left', icon: <FiChevronLeft /> },
  { id: 'ghrm-icon-chevron-right', icon: <FiChevronRight /> },
  { id: 'ghrm-icon-home', icon: <GoHome /> },
  { id: 'ghrm-icon-note', icon: <FaRegNoteSticky /> },
  { id: 'ghrm-icon-plus', icon: <FiPlus /> },
  { id: 'ghrm-icon-edit', icon: <TbEdit /> },
  { id: 'ghrm-icon-trash', icon: <GoTrash /> },
  { id: 'ghrm-icon-tip', icon: <GoLightBulb /> },
  { id: 'ghrm-icon-important', icon: <FaRegCommentDots /> },
  { id: 'ghrm-icon-warning', icon: <GoAlert /> },
  { id: 'ghrm-icon-caution', icon: <GoStop /> },
  { id: 'ghrm-icon-external', icon: <RxExternalLink /> },
  { id: 'ghrm-icon-logout', icon: <MdLogout /> },
  { id: 'ghrm-icon-cloud-offline', icon: <IoCloudOfflineOutline /> },
];
